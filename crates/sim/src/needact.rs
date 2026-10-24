//! Need actions: eating, sleeping, and wearing clothes (R-011, R-012, R-001).
//!
//! A need action replaces work (D-040). A colonist with a need action takes no job. When a
//! need starts an action, the colonist stops its job. Food and clothes stacks are reserved
//! with `Item::reserved_by`, so no other colonist can take them. A bed is reserved by the
//! sleeping action itself: no two colonists have the same bed.

use crate::Sim;
use crate::colonist::{ColonistId, NeedAct};
use crate::item::{ItemId, ItemLoc};
use crate::job::{AbortReason, Leg, octile};
use crate::life::MILLI;
use base::content::BuildingKind;

/// A colonist rests in the shelter for this many retry periods before it looks again.
const SHELTER_REST: u64 = 6;

/// Sort key of a food stack: rank, rot bucket, distance, item index.
type FoodKey = (usize, u32, u32, u32);

/// The best candidate so far: sort key, stack, and tile.
type Pick<K> = Option<((K, u32, u32), ItemId, (u16, u16))>;

impl Sim {
    /// True if the needs system works.
    pub(crate) fn needs_active(&self) -> bool {
        self.rules.needs.enabled && !self.needs_off
    }

    /// Advance the need actions, then start new ones. Runs before the job system.
    pub(crate) fn update_needs(&mut self) {
        if !self.needs_active() {
            return;
        }
        let ids: Vec<ColonistId> = self.colonists.iter().map(|(id, _)| id).collect();
        for &id in &ids {
            self.advance_act(id);
        }
        for &id in &ids {
            self.choose_act(id);
        }
    }

    /// End the need action of a colonist. The colonist looks for a new one after `delay`
    /// ticks.
    pub(crate) fn cancel_act(&mut self, id: ColonistId, delay: u64) {
        let tick = self.tick;
        let Some(c) = self.colonists.get_mut(id) else {
            return;
        };
        let act = c.act.take();
        c.need_next = tick + delay;
        if let Some(NeedAct::Eat { item, .. } | NeedAct::Wear { item, .. }) = act
            && self
                .items
                .get(item)
                .is_some_and(|i| i.reserved_by == Some(id))
        {
            self.items.set_reserved(item, None);
        }
    }

    fn set_act(&mut self, id: ColonistId, act: NeedAct) {
        if let Some(c) = self.colonists.get_mut(id) {
            c.act = Some(act);
        }
    }

    /// The colonist lies down where it is, or on the bed tile.
    fn lie_down(&mut self, id: ColonistId, bed: Option<(u16, u16)>) {
        self.path_queue.retain(|r| r.colonist != id);
        if let Some(c) = self.colonists.get_mut(id) {
            c.path.clear();
            c.target = None;
            c.progress = 0;
            c.act = Some(NeedAct::Sleep { bed, asleep: true });
        }
    }

    fn advance_act(&mut self, id: ColonistId) {
        let Some(act) = self.colonists.get(id).and_then(|c| c.act) else {
            return;
        };
        let retry = u64::from(self.rules.needs.need_retry_ticks);
        match act {
            NeedAct::Eat {
                item,
                stand,
                progress,
            } => {
                if !self.reserved_stack_ok(id, item, stand, true) {
                    self.cancel_act(id, 0);
                    return;
                }
                match progress {
                    None => match self.leg_state(id, stand) {
                        Leg::Arrived => self.set_act(
                            id,
                            NeedAct::Eat {
                                item,
                                stand,
                                progress: Some(0),
                            },
                        ),
                        Leg::Walking => {}
                        Leg::Failed => self.cancel_act(id, retry),
                    },
                    Some(done) => {
                        if done + 1 >= self.rules.needs.eat_ticks {
                            self.finish_eat(id, item);
                        } else {
                            self.set_act(
                                id,
                                NeedAct::Eat {
                                    item,
                                    stand,
                                    progress: Some(done + 1),
                                },
                            );
                        }
                    }
                }
            }
            NeedAct::Wear { item, stand } => {
                if !self.reserved_stack_ok(id, item, stand, false) {
                    self.cancel_act(id, 0);
                    return;
                }
                match self.leg_state(id, stand) {
                    Leg::Arrived => self.finish_wear(id, item, stand),
                    Leg::Walking => {}
                    Leg::Failed => self.cancel_act(id, retry),
                }
            }
            NeedAct::Shelter { stand } => match self.leg_state(id, stand) {
                Leg::Arrived => self.cancel_act(id, retry * SHELTER_REST),
                Leg::Walking => {}
                Leg::Failed => self.cancel_act(id, retry),
            },
            NeedAct::Sleep { bed, asleep } => {
                if asleep {
                    return;
                }
                let Some(bed) = bed else {
                    self.lie_down(id, None);
                    return;
                };
                let exhausted = self.colonists.get(id).is_some_and(|c| c.rest <= 0);
                if exhausted || !self.is_bed_tile(bed.0, bed.1) {
                    self.lie_down(id, None);
                    return;
                }
                match self.leg_state(id, bed) {
                    Leg::Arrived => self.lie_down(id, Some(bed)),
                    Leg::Walking => {}
                    Leg::Failed => self.lie_down(id, None),
                }
            }
        }
    }

    /// True if the stack is still reserved by the colonist, lies on its tile, and is food
    /// (`food`) or clothes.
    fn reserved_stack_ok(
        &self,
        id: ColonistId,
        item: ItemId,
        stand: (u16, u16),
        food: bool,
    ) -> bool {
        let n = &self.rules.needs;
        self.items.get(item).is_some_and(|i| {
            i.reserved_by == Some(id)
                && i.loc == ItemLoc::Tile(stand.0, stand.1)
                && if food {
                    n.food(i.material).is_some() && !self.is_rotten(i)
                } else {
                    n.clothes_def(i.material).is_some()
                }
        })
    }

    fn finish_eat(&mut self, id: ColonistId, item: ItemId) {
        let n = &self.rules.needs;
        let Some(food) = self
            .items
            .get(item)
            .and_then(|i| n.food(i.material))
            .copied()
        else {
            self.cancel_act(id, 0);
            return;
        };
        let max = i32::try_from(n.hunger_max).unwrap_or(i32::MAX) * MILLI;
        let gain = i32::try_from(food.nutrition).unwrap_or(i32::MAX) * MILLI;
        if let Some((material, rot)) = self.items.get(item).map(|i| (i.material, i.rot)) {
            self.eat_disease_roll(id, material, rot);
        }
        self.items.take_one(item);
        if let Some(c) = self.colonists.get_mut(id) {
            c.hunger = c.hunger.saturating_add(gain).min(max);
            c.act = None;
            c.need_next = self.tick;
        }
        self.give_memory(id, food.memory);
    }

    fn finish_wear(&mut self, id: ColonistId, item: ItemId, stand: (u16, u16)) {
        let Some(material) = self.items.get(item).map(|i| i.material) else {
            self.cancel_act(id, 0);
            return;
        };
        self.items.take_one(item);
        let old = self.colonists.get(id).and_then(|c| c.clothes);
        if let Some(c) = self.colonists.get_mut(id) {
            c.clothes = Some(material);
            c.act = None;
            c.need_next = self.tick;
        }
        if let Some(old) = old {
            self.items.spawn(old, 1, stand.0, stand.1, false);
        }
    }

    fn choose_act(&mut self, id: ColonistId) {
        let Some(c) = self.colonists.get(id) else {
            return;
        };
        if c.act.is_some() || c.need_next > self.tick || c.drafted || c.foe.is_some() {
            return;
        }
        let n = &self.rules.needs;
        let hungry = c.hunger <= i32::try_from(n.hunger_eat_at).unwrap_or(0) * MILLI;
        let tired = c.rest <= i32::try_from(n.rest_sleep_at).unwrap_or(0) * MILLI;
        let retry = u64::from(n.need_retry_ticks);
        if hungry && let Some((item, stand)) = self.find_food(id) {
            self.begin_item_act(
                id,
                item,
                stand,
                NeedAct::Eat {
                    item,
                    stand,
                    progress: None,
                },
            );
            return;
        }
        if tired {
            self.begin_sleep(id);
            return;
        }
        if let Some((item, stand)) = self.find_better_clothes(id) {
            self.begin_item_act(id, item, stand, NeedAct::Wear { item, stand });
            return;
        }
        if self.wants_shelter(id)
            && let Some(stand) = self.find_shelter(id)
        {
            self.abort_job(id, AbortReason::JobGone, false);
            self.set_act(id, NeedAct::Shelter { stand });
            self.start_leg(id, stand);
            return;
        }
        if let Some(c) = self.colonists.get_mut(id) {
            c.need_next = self.tick + retry;
        }
    }

    /// Stop the job, reserve the stack, and walk to it.
    fn begin_item_act(&mut self, id: ColonistId, item: ItemId, stand: (u16, u16), act: NeedAct) {
        self.abort_job(id, AbortReason::JobGone, false);
        self.items.set_reserved(item, Some(id));
        self.set_act(id, act);
        self.start_leg(id, stand);
    }

    fn begin_sleep(&mut self, id: ColonistId) {
        self.abort_job(id, AbortReason::JobGone, false);
        match self.find_bed(id) {
            Some(bed) => {
                self.set_act(
                    id,
                    NeedAct::Sleep {
                        bed: Some(bed),
                        asleep: false,
                    },
                );
                self.start_leg(id, bed);
            }
            None => self.lie_down(id, None),
        }
    }

    /// The best food stack the colonist can reach: the best food first (file order), then the
    /// older food (R-005), then the nearest stack, then the lowest item index. Rotten food
    /// is never chosen.
    fn find_food(&self, id: ColonistId) -> Option<(ItemId, (u16, u16))> {
        let c = self.colonists.get(id)?;
        let from = c.tile();
        let n = &self.rules.needs;
        let mut best: Option<(FoodKey, ItemId, (u16, u16))> = None;
        for (item_id, item) in self.items.iter() {
            let ItemLoc::Tile(x, y) = item.loc else {
                continue;
            };
            if item.reserved_by.is_some() {
                continue;
            }
            if self.is_rotten(item) {
                continue;
            }
            let Some(rank) = n.foods.iter().position(|f| f.material == item.material) else {
                continue;
            };
            // R-005: eat the older food first (bucket of rot, 10 per cent steps).
            let bucket = u32::MAX - item.rot / (self.rules.spoil.rotten_at / 10).max(1);
            let key = (rank, bucket, octile(from, (x, y)), item_id.index());
            if best.as_ref().is_some_and(|b| b.0 <= key) {
                continue;
            }
            if !self.regions.connected(
                (i32::from(from.0), i32::from(from.1)),
                (i32::from(x), i32::from(y)),
            ) {
                continue;
            }
            best = Some((key, item_id, (x, y)));
        }
        best.map(|(_, i, t)| (i, t))
    }

    /// Clothes on the ground that warm better than the worn clothes: the warmest first, then
    /// the nearest, then the lowest item index.
    fn find_better_clothes(&self, id: ColonistId) -> Option<(ItemId, (u16, u16))> {
        let c = self.colonists.get(id)?;
        let from = c.tile();
        let n = &self.rules.needs;
        let worn = c
            .clothes
            .and_then(|m| n.clothes_def(m))
            .map_or(-1, |d| i32::from(d.insulation));
        let mut best: Pick<i32> = None;
        for (item_id, item) in self.items.iter() {
            let ItemLoc::Tile(x, y) = item.loc else {
                continue;
            };
            let Some(def) = n.clothes_def(item.material) else {
                continue;
            };
            if item.reserved_by.is_some() || i32::from(def.insulation) <= worn {
                continue;
            }
            let key = (
                -i32::from(def.insulation),
                octile(from, (x, y)),
                item_id.index(),
            );
            if best.as_ref().is_some_and(|b| b.0 <= key) {
                continue;
            }
            if !self.regions.connected(
                (i32::from(from.0), i32::from(from.1)),
                (i32::from(x), i32::from(y)),
            ) {
                continue;
            }
            best = Some((key, item_id, (x, y)));
        }
        best.map(|(_, i, t)| (i, t))
    }

    /// True if the colonist stands outdoors in a temperature outside its comfort band, and
    /// has no job or already suffers from it.
    fn wants_shelter(&self, id: ColonistId) -> bool {
        let Some(c) = self.colonists.get(id) else {
            return false;
        };
        if self.rooms.index_at(c.x, c.y).is_some() {
            return false;
        }
        let band = crate::life::comfort_band(&self.rules, c);
        let outdoor = i32::from(self.outdoor_temp());
        if outdoor >= band.low && outdoor <= band.high {
            return false;
        }
        let n = &self.rules.needs;
        let suffers =
            c.hypothermia.max(c.heatstroke) >= i32::try_from(n.hazard_limit / 2).unwrap_or(0);
        c.job.is_none() || suffers
    }

    /// The nearest walkable tile of the room that is nearest to the comfort band of the
    /// colonist and better than the open air, in its region. Ties by distance, row, column.
    fn find_shelter(&self, id: ColonistId) -> Option<(u16, u16)> {
        let c = self.colonists.get(id)?;
        let from = c.tile();
        let band = crate::life::comfort_band(&self.rules, c);
        let off = |t: i32| (band.low - t).max(t - band.high).max(0);
        let outside = off(i32::from(self.outdoor_temp()));
        let mut best: Option<(i32, u32, u16, u16)> = None;
        for room in self.rooms.iter() {
            let Some(&(rx, ry)) = room.tiles.first() else {
                continue;
            };
            // A room helps if it is nearer to the comfort band than the open air.
            let miss = off(i32::from(self.room_temp(rx, ry).unwrap_or(0)));
            if miss >= outside {
                continue;
            }
            for &(x, y) in &room.tiles {
                if !self.map.passable(i32::from(x), i32::from(y)) {
                    continue;
                }
                let key = (miss, octile(from, (x, y)), y, x);
                if best.is_some_and(|b| b <= key) {
                    continue;
                }
                if self.regions.connected(
                    (i32::from(from.0), i32::from(from.1)),
                    (i32::from(x), i32::from(y)),
                ) {
                    best = Some(key);
                }
            }
        }
        best.map(|(_, _, y, x)| (x, y))
    }

    /// The nearest free bed in the region of the colonist, ties by row then column.
    fn find_bed(&self, id: ColonistId) -> Option<(u16, u16)> {
        let c = self.colonists.get(id)?;
        let from = c.tile();
        let taken: Vec<(u16, u16)> = self
            .colonists
            .iter()
            .filter(|(other, _)| *other != id)
            .filter_map(|(_, o)| match o.act {
                Some(NeedAct::Sleep { bed, .. }) => bed,
                _ => None,
            })
            .collect();
        let beds = self.buildings.object_tiles(|def| {
            self.rules
                .building(def)
                .is_some_and(|d| d.kind == BuildingKind::Bed)
        });
        let free: Vec<(u16, u16)> = beds.into_iter().filter(|t| !taken.contains(t)).collect();
        self.sick_room_filter(id, free)
            .into_iter()
            .filter(|t| {
                self.regions.connected(
                    (i32::from(from.0), i32::from(from.1)),
                    (i32::from(t.0), i32::from(t.1)),
                )
            })
            .min_by_key(|&t| (octile(from, t), t.1, t.0))
    }
}
