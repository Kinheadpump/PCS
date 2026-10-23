//! Items: stacks of one material, on a tile or carried by a colonist.
//!
//! Items live in an arena with stable IDs. A derived index finds the items on a tile.
//! The index is rebuilt after a load.

use crate::colonist::ColonistId;
use base::{Arena, Id};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Typed ID of an item.
pub type ItemId = Id<Item>;

/// Where an item is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemLoc {
    /// On the tile `(x, y)`.
    Tile(u16, u16),
    /// Carried by a colonist.
    Carried(ColonistId),
}

/// A stack of items of one material.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// Material index (`materials.ron` order).
    pub material: u16,
    /// Number of units in the stack. Always 1 or more.
    pub count: u32,
    /// Where the stack is.
    pub loc: ItemLoc,
    /// The colonist whose job reserves this stack. Only one colonist can.
    pub reserved_by: Option<ColonistId>,
    /// True if a hauling job must bring this stack to a stockpile.
    pub to_haul: bool,
    /// Rot value in thousandths (R-005). 0 is fresh. At the rotten limit of `spoilage.ron`
    /// the stack is rotten. Only food gains rot. Stacks that merge keep the weighted mean.
    pub rot: u32,
}

/// The rot value of two merged stacks: the mean weighted by the units, rounded up.
fn mean_rot(rot_a: u32, count_a: u32, rot_b: u32, count_b: u32) -> u32 {
    let total = u64::from(count_a) + u64::from(count_b);
    if total == 0 {
        return 0;
    }
    let sum = u64::from(rot_a) * u64::from(count_a) + u64::from(rot_b) * u64::from(count_b);
    u32::try_from(sum.div_ceil(total)).unwrap_or(u32::MAX)
}

/// All items of the colony.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Items {
    arena: Arena<Item>,
    /// Derived cache: item IDs on each tile, in insertion order.
    #[serde(skip)]
    by_tile: BTreeMap<(u16, u16), Vec<ItemId>>,
}

impl Items {
    /// Put a new stack on a tile. A count of 0 makes no item.
    pub fn spawn(
        &mut self,
        material: u16,
        count: u32,
        x: u16,
        y: u16,
        to_haul: bool,
    ) -> Option<ItemId> {
        if count == 0 {
            return None;
        }
        let id = self.arena.insert(Item {
            material,
            count,
            loc: ItemLoc::Tile(x, y),
            reserved_by: None,
            to_haul,
            rot: 0,
        });
        self.by_tile.entry((x, y)).or_default().push(id);
        Some(id)
    }

    /// One item.
    pub fn get(&self, id: ItemId) -> Option<&Item> {
        self.arena.get(id)
    }

    /// All items, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (ItemId, &Item)> {
        self.arena.iter()
    }

    /// Number of item stacks.
    pub fn len(&self) -> usize {
        self.arena.len()
    }

    /// True if there are no items.
    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    /// The items that lie on a tile, in insertion order.
    pub fn at_tile(&self, x: u16, y: u16) -> &[ItemId] {
        self.by_tile.get(&(x, y)).map_or(&[], Vec::as_slice)
    }

    /// Total units of one material in all stacks (on tiles and carried).
    pub fn total_units(&self, material: u16) -> u64 {
        self.arena
            .iter()
            .filter(|(_, i)| i.material == material)
            .map(|(_, i)| u64::from(i.count))
            .sum()
    }

    /// Total units of all materials.
    pub fn all_units(&self) -> u64 {
        self.arena.iter().map(|(_, i)| u64::from(i.count)).sum()
    }

    /// Set or clear the reservation. Return false if the item does not exist.
    pub(crate) fn set_reserved(&mut self, id: ItemId, by: Option<ColonistId>) -> bool {
        match self.arena.get_mut(id) {
            Some(item) => {
                item.reserved_by = by;
                true
            }
            None => false,
        }
    }

    /// Set the haul flag. Return false if the item does not exist.
    pub(crate) fn set_to_haul(&mut self, id: ItemId, to_haul: bool) -> bool {
        match self.arena.get_mut(id) {
            Some(item) => {
                item.to_haul = to_haul;
                true
            }
            None => false,
        }
    }

    /// A colonist picks the stack up. It leaves its tile. Return false if the stack is
    /// not on a tile.
    pub(crate) fn pick_up(&mut self, id: ItemId, by: ColonistId) -> bool {
        let Some(item) = self.arena.get_mut(id) else {
            return false;
        };
        let ItemLoc::Tile(x, y) = item.loc else {
            return false;
        };
        item.loc = ItemLoc::Carried(by);
        self.unindex(id, (x, y));
        true
    }

    /// A colonist puts the carried stack down on a tile. If the tile holds an unreserved
    /// stack of the same material with room (`stack_max`), the units join that stack
    /// and this stack ends. Return the ID of the stack that now holds the units.
    pub(crate) fn put_down(
        &mut self,
        id: ItemId,
        x: u16,
        y: u16,
        stack_max: u32,
    ) -> Option<ItemId> {
        let (material, count, rot) = {
            let item = self.arena.get(id)?;
            if !matches!(item.loc, ItemLoc::Carried(_)) {
                return None;
            }
            (item.material, item.count, item.rot)
        };
        let target = self.at_tile(x, y).iter().copied().find(|&other| {
            self.arena.get(other).is_some_and(|o| {
                o.material == material
                    && o.reserved_by.is_none()
                    && o.count.saturating_add(count) <= stack_max
            })
        });
        if let Some(other) = target {
            if let Some(o) = self.arena.get_mut(other) {
                o.rot = mean_rot(o.rot, o.count, rot, count);
                o.count += count;
            }
            self.arena.remove(id);
            return Some(other);
        }
        if let Some(item) = self.arena.get_mut(id) {
            item.loc = ItemLoc::Tile(x, y);
            item.reserved_by = None;
        }
        self.by_tile.entry((x, y)).or_default().push(id);
        Some(id)
    }

    /// Take a stack out of the world (a building uses its units). Return the stack.
    pub(crate) fn remove(&mut self, id: ItemId) -> Option<Item> {
        let item = self.arena.remove(id)?;
        if let ItemLoc::Tile(x, y) = item.loc {
            self.unindex(id, (x, y));
        }
        Some(item)
    }

    /// Cut `count` units from a stack on a tile into a new stack on the same tile. The new
    /// stack has no reservation and keeps the haul flag. Return `None` if the stack has
    /// `count` units or fewer, is not on a tile, or `count` is 0.
    pub(crate) fn split_off(&mut self, id: ItemId, count: u32) -> Option<ItemId> {
        let item = self.arena.get_mut(id)?;
        let ItemLoc::Tile(x, y) = item.loc else {
            return None;
        };
        if count == 0 || item.count <= count {
            return None;
        }
        item.count -= count;
        let (material, to_haul, rot) = (item.material, item.to_haul, item.rot);
        let new = self.spawn(material, count, x, y, to_haul)?;
        if let Some(n) = self.arena.get_mut(new) {
            n.rot = rot;
        }
        Some(new)
    }

    /// Set the rot value of a stack. Return false if the stack does not exist.
    pub(crate) fn set_rot(&mut self, id: ItemId, rot: u32) -> bool {
        match self.arena.get_mut(id) {
            Some(item) => {
                item.rot = rot;
                true
            }
            None => false,
        }
    }

    /// Take one unit from a stack on a tile and free its reservation. A stack of 1 unit
    /// ends. Return false if the stack does not exist.
    pub(crate) fn take_one(&mut self, id: ItemId) -> bool {
        let Some(item) = self.arena.get_mut(id) else {
            return false;
        };
        if item.count > 1 {
            item.count -= 1;
            item.reserved_by = None;
            return true;
        }
        self.remove(id).is_some()
    }

    /// Move a stack that lies on a tile to another tile. Return false if the stack does
    /// not lie on a tile.
    pub(crate) fn relocate(&mut self, id: ItemId, x: u16, y: u16) -> bool {
        let Some(item) = self.arena.get_mut(id) else {
            return false;
        };
        let ItemLoc::Tile(ox, oy) = item.loc else {
            return false;
        };
        item.loc = ItemLoc::Tile(x, y);
        self.unindex(id, (ox, oy));
        self.by_tile.entry((x, y)).or_default().push(id);
        true
    }

    /// Rebuild the tile index from the arena. Call after a load.
    pub(crate) fn rebuild_index(&mut self) {
        self.by_tile.clear();
        let placed: Vec<(ItemId, (u16, u16))> = self
            .arena
            .iter()
            .filter_map(|(id, i)| match i.loc {
                ItemLoc::Tile(x, y) => Some((id, (x, y))),
                ItemLoc::Carried(_) => None,
            })
            .collect();
        for (id, tile) in placed {
            self.by_tile.entry(tile).or_default().push(id);
        }
    }

    fn unindex(&mut self, id: ItemId, tile: (u16, u16)) {
        if let Some(list) = self.by_tile.get_mut(&tile) {
            list.retain(|&other| other != id);
            if list.is_empty() {
                self.by_tile.remove(&tile);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_pick_up_and_put_down_keep_the_units() {
        let mut items = Items::default();
        let a = items.spawn(1, 5, 2, 3, true).unwrap();
        assert!(items.spawn(1, 0, 2, 3, true).is_none());
        assert_eq!(items.at_tile(2, 3), &[a]);
        let who =
            Arena::<crate::colonist::Colonist>::new().insert(crate::colonist::Colonist::new(0, 0));
        assert!(items.pick_up(a, who));
        assert!(items.at_tile(2, 3).is_empty());
        assert!(!items.pick_up(a, who), "already carried");
        let b = items.spawn(1, 10, 7, 7, false).unwrap();
        let kept = items.put_down(a, 7, 7, 20).unwrap();
        assert_eq!(kept, b, "merged into the stack on the tile");
        assert_eq!(items.get(b).unwrap().count, 15);
        assert_eq!(items.total_units(1), 15);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn a_full_stack_does_not_take_more() {
        let mut items = Items::default();
        let a = items.spawn(1, 15, 0, 0, true).unwrap();
        let who =
            Arena::<crate::colonist::Colonist>::new().insert(crate::colonist::Colonist::new(0, 0));
        assert!(items.pick_up(a, who));
        let _b = items.spawn(1, 10, 1, 1, false).unwrap();
        let kept = items.put_down(a, 1, 1, 20).unwrap();
        assert_eq!(kept, a, "no room: both stacks stay");
        assert_eq!(items.at_tile(1, 1).len(), 2);
    }
}
