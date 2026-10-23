//! Disease and medicine (S17, R-060 to R-064).
//!
//! A colonist can have food poisoning, wound infection, and winter fever. Each one has a
//! severity from 0 to 100. Once each game day (at midnight) the severity changes by a number
//! from `disease.ron`: one value with a treatment, one after a night in a bed, one otherwise.
//! At 0 the disease ends. Above the health limit a disease costs health each game hour.
//! A doctor job treats a sick or wounded colonist that sleeps in a bed. It is a mine job with
//! the argument `DOCTOR_ARG`, so the game shows it with the other mine jobs. The disease layer
//! has its own random stream, so a colony that stays healthy does not change.
//!
//! No new disease starts in the first `grace_days` game days of a colony.
//!
//! Gaps for later sprints: the doctor takes the medicine from any stack with no walk, the
//! doctor job follows the Mine priority, and nothing but a debug command wounds a colonist.

use crate::Sim;
use crate::colonist::{ColonistId, NeedAct};
use crate::item::ItemLoc;
use crate::job::{AbortReason, ActiveJob, Job, JobKind, Reject};
use crate::life::{HealthCause, HealthPart};
use crate::skill::Skill;
use crate::time::{TICKS_PER_DAY, TICKS_PER_HOUR};
use base::Pcg32;
use base::content::DiseaseDef;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The `arg` of a mine job that is a doctor job. The target tile is the bed of the patient.
pub const DOCTOR_ARG: u16 = 2;

/// A disease type (R-060).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DiseaseKind {
    /// Bad food (R-061).
    FoodPoisoning,
    /// An infected wound (R-062).
    WoundInfection,
    /// A contagious fever of the cold season (R-063).
    WinterFever,
}

impl DiseaseKind {
    /// All types, in index order.
    pub const ALL: [DiseaseKind; 3] = [
        DiseaseKind::FoodPoisoning,
        DiseaseKind::WoundInfection,
        DiseaseKind::WinterFever,
    ];

    /// Index in `ALL`.
    pub fn index(self) -> u8 {
        match self {
            DiseaseKind::FoodPoisoning => 0,
            DiseaseKind::WoundInfection => 1,
            DiseaseKind::WinterFever => 2,
        }
    }

    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            DiseaseKind::FoodPoisoning => "Food poisoning",
            DiseaseKind::WoundInfection => "Wound infection",
            DiseaseKind::WinterFever => "Winter fever",
        }
    }
}

/// One disease of one colonist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Illness {
    /// The type.
    pub kind: DiseaseKind,
    /// Severity, 1 to 100.
    pub severity: u32,
}

/// Where a part of the daily severity change comes from (R-060).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IllnessCause {
    /// No bed and no treatment.
    Untreated,
    /// A night in a bed (a warm bed for winter fever).
    Bed,
    /// A treatment is active.
    Treatment,
}

impl IllnessCause {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            IllnessCause::Untreated => "No bed, no treatment",
            IllnessCause::Bed => "Night in a bed",
            IllnessCause::Treatment => "Treatment",
        }
    }
}

/// One named contribution to the severity change of the next game day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IllnessPart {
    /// What it is.
    pub cause: IllnessCause,
    /// Severity points. Negative is better.
    pub delta: i32,
}

/// A disease of a colonist with the why chain (R-060).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IllnessReport {
    /// The type.
    pub kind: DiseaseKind,
    /// Severity now.
    pub severity: u32,
    /// Work speed change in percent points.
    pub speed_pct: i32,
    /// The parts of the next daily change. Their sum is `change`.
    pub parts: Vec<IllnessPart>,
    /// Sum of the parts.
    pub change: i32,
}

/// A thing that happened with disease. The caller takes it with `Sim::drain_disease_events`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiseaseEvent {
    /// A colonist got a disease.
    Fell {
        /// The type.
        kind: DiseaseKind,
        /// Where.
        x: u16,
        /// Where.
        y: u16,
    },
    /// A disease ended.
    Recovered {
        /// The type.
        kind: DiseaseKind,
        /// Where.
        x: u16,
        /// Where.
        y: u16,
    },
    /// A doctor job ended.
    Treated {
        /// Where the patient lies.
        x: u16,
        /// Where the patient lies.
        y: u16,
        /// False if the treatment failed.
        success: bool,
    },
}

/// What the disease system knows about one colonist. Part of the state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Patient {
    illnesses: Vec<Illness>,
    /// No winter fever before this tick.
    immune_until: u64,
    /// A treatment lasts until this tick.
    treated_until: u64,
    /// Slept in a bed since the last midnight.
    slept_bed: bool,
    /// The bed was warm (inside or above the comfort band).
    slept_warm: bool,
    /// The room where the colonist slept: its first tile.
    slept_room: Option<(u16, u16)>,
    /// The room was inside the comfort band.
    room_comfort: bool,
}

impl Patient {
    fn has(&self, kind: DiseaseKind) -> bool {
        self.illnesses.iter().any(|i| i.kind == kind)
    }

    fn is_idle(&self, tick: u64) -> bool {
        self.illnesses.is_empty() && self.immune_until <= tick && self.treated_until <= tick
    }
}

/// The disease state: the patients, the sick rooms, and the random stream. Part of the state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct DiseaseState {
    rng: Pcg32,
    patients: BTreeMap<ColonistId, Patient>,
    /// The first tile of each sick room.
    sick_rooms: BTreeSet<(u16, u16)>,
}

impl DiseaseState {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            rng: Pcg32::from_seed(seed ^ 0x4449_5345_4153_4531),
            patients: BTreeMap::new(),
            sick_rooms: BTreeSet::new(),
        }
    }
}

impl Job {
    /// True if this is a mine job that is a doctor job.
    pub fn is_doctor(&self) -> bool {
        self.kind == JobKind::Mine && self.arg == DOCTOR_ARG
    }
}

fn def_of(rules: &base::content::DiseaseRules, kind: DiseaseKind) -> DiseaseDef {
    match kind {
        DiseaseKind::FoodPoisoning => rules.food_poisoning,
        DiseaseKind::WoundInfection => rules.wound_infection,
        DiseaseKind::WinterFever => rules.winter_fever,
    }
}

impl Sim {
    // ---- queries ----

    /// The diseases of a colonist, with the why chain of each daily change (R-060).
    pub fn illnesses(&self, id: ColonistId) -> Vec<IllnessReport> {
        let Some(p) = self.disease.patients.get(&id) else {
            return Vec::new();
        };
        let rules = &self.rules.disease;
        p.illnesses
            .iter()
            .map(|ill| {
                let def = def_of(rules, ill.kind);
                let (cause, delta) = self.daily_change(p, ill.kind, def);
                IllnessReport {
                    kind: ill.kind,
                    severity: ill.severity,
                    speed_pct: def.speed_pct,
                    parts: vec![IllnessPart { cause, delta }],
                    change: delta,
                }
            })
            .collect()
    }

    /// True if the colonist has any disease.
    pub fn is_sick(&self, id: ColonistId) -> bool {
        self.disease
            .patients
            .get(&id)
            .is_some_and(|p| !p.illnesses.is_empty())
    }

    /// Number of colonists with a disease.
    pub fn sick_count(&self) -> usize {
        self.disease
            .patients
            .iter()
            .filter(|(id, p)| !p.illnesses.is_empty() && self.colonists.contains(**id))
            .count()
    }

    /// True if a treatment of the colonist is active.
    pub fn is_treated(&self, id: ColonistId) -> bool {
        self.disease
            .patients
            .get(&id)
            .is_some_and(|p| p.treated_until > self.tick)
    }

    /// True if the room that contains the tile is a sick room (R-064).
    pub fn is_sick_room(&self, x: u16, y: u16) -> bool {
        self.room_key(x, y)
            .is_some_and(|k| self.disease.sick_rooms.contains(&k))
    }

    /// Take the disease events that the last ticks made.
    pub fn drain_disease_events(&mut self) -> Vec<DiseaseEvent> {
        std::mem::take(&mut self.disease_events)
    }

    /// The chance in permille of food poisoning for a colonist that eats a stack (R-061).
    pub fn food_poisoning_permille(&self, id: ColonistId, material: u16, rot: u32) -> u32 {
        let r = &self.rules.disease;
        let raw = self.rules.raw_foods.contains(&material);
        let mut permille = if raw {
            r.food_raw_pct * 10
        } else {
            let level = u32::from(self.skill_level(id, Skill::Cooking).unwrap_or(0));
            (r.food_cooked_pct * 10).saturating_sub(r.food_cook_skill_permille * level)
        };
        let limit = u64::from(self.rules.spoil.rotten_at);
        if u64::from(rot) * 100 >= limit * u64::from(r.food_rot_value) {
            permille += r.food_rot_pct * 10;
        }
        permille.min(1000)
    }

    fn room_key(&self, x: u16, y: u16) -> Option<(u16, u16)> {
        self.room_at(x, y).and_then(|r| r.tiles.first().copied())
    }

    /// The severity change of the next game day for one disease.
    fn daily_change(&self, p: &Patient, kind: DiseaseKind, def: DiseaseDef) -> (IllnessCause, i32) {
        if p.treated_until > self.tick {
            return (IllnessCause::Treatment, def.treated);
        }
        let bed = p.slept_bed && (kind != DiseaseKind::WinterFever || p.slept_warm);
        if bed {
            (IllnessCause::Bed, def.bed)
        } else {
            (IllnessCause::Untreated, def.untreated)
        }
    }

    fn grace_over(&self) -> bool {
        self.tick >= u64::from(self.rules.disease.grace_days) * TICKS_PER_DAY
    }

    // ---- hooks ----

    /// Health part of a disease above the severity limit (R-060).
    pub(crate) fn disease_health_part(&self, id: ColonistId) -> Option<HealthPart> {
        let r = &self.rules.disease;
        let p = self.disease.patients.get(&id)?;
        p.illnesses
            .iter()
            .any(|i| i.severity > r.health_loss_above)
            .then(|| HealthPart {
                cause: HealthCause::Disease,
                per_hour: -i32::try_from(r.health_loss_per_hour).unwrap_or(i32::MAX),
            })
    }

    /// Work speed changes of the diseases of a colonist: the kind index and the percent.
    pub(crate) fn disease_speed_mods(&self, id: ColonistId) -> Vec<(u8, i32)> {
        let Some(p) = self.disease.patients.get(&id) else {
            return Vec::new();
        };
        p.illnesses
            .iter()
            .map(|i| {
                (
                    i.kind.index(),
                    def_of(&self.rules.disease, i.kind).speed_pct,
                )
            })
            .filter(|&(_, pct)| pct != 0)
            .collect()
    }

    /// A colonist eats a stack: roll for food poisoning (R-061).
    pub(crate) fn eat_disease_roll(&mut self, id: ColonistId, material: u16, rot: u32) {
        if !self.grace_over() {
            return;
        }
        let permille = self.food_poisoning_permille(id, material, rot);
        if permille > 0 && self.disease.rng.chance(permille, 1000) {
            self.fall_ill(id, DiseaseKind::FoodPoisoning);
        }
    }

    /// Give a colonist a disease at its start severity. Return false if the colonist has
    /// the disease already or cannot get it.
    fn fall_ill(&mut self, id: ColonistId, kind: DiseaseKind) -> bool {
        let Some(c) = self.colonists.get(id) else {
            return false;
        };
        let (x, y) = c.tile();
        let start = def_of(&self.rules.disease, kind).start;
        let tick = self.tick;
        let p = self.disease.patients.entry(id).or_default();
        if p.has(kind) || (kind == DiseaseKind::WinterFever && p.immune_until > tick) {
            return false;
        }
        p.illnesses.push(Illness {
            kind,
            severity: start,
        });
        p.illnesses.sort_by_key(|i| i.kind);
        self.disease_events.push(DiseaseEvent::Fell { kind, x, y });
        true
    }

    /// Each tick after the tick number grew. Each game hour: record the sleep, roll the cold,
    /// and ask for doctor jobs. Each game day at midnight: change the severities.
    pub(crate) fn disease_tick(&mut self) {
        if !self.tick.is_multiple_of(TICKS_PER_HOUR) || !self.needs_active() {
            return;
        }
        self.disease_hour();
        if self.tick.is_multiple_of(TICKS_PER_DAY) {
            self.disease_day();
        }
    }

    fn disease_hour(&mut self) {
        let ids: Vec<ColonistId> = self.colonists.iter().map(|(id, _)| id).collect();
        let cold_permille = self.rules.disease.cold_catch_permille;
        for id in ids {
            let Some(c) = self.colonists.get(id) else {
                continue;
            };
            let (x, y) = c.tile();
            let sleep = match c.act {
                Some(NeedAct::Sleep { bed, asleep: true }) => Some(bed),
                _ => None,
            };
            let low = self.comfort_band(id).map_or(0, |b| b.low);
            let temp = self.tile_temperature(x, y).map(|t| i32::from(t.total));
            let high = self.rules.needs.comfort_high;
            let comfort = temp.is_some_and(|t| t >= low && t <= i32::from(high));
            let room = self.room_key(x, y);
            if let Some(bed) = sleep {
                let p = self.disease.patients.entry(id).or_default();
                if bed.is_some() {
                    p.slept_bed = true;
                }
                if temp.is_some_and(|t| t >= low) {
                    p.slept_warm = true;
                }
                p.slept_room = room;
                p.room_comfort = comfort;
            }
            let cold = temp.is_some_and(|t| t < low);
            if cold && self.grace_over() && cold_permille > 0 {
                let immune = self
                    .disease
                    .patients
                    .get(&id)
                    .is_some_and(|p| p.immune_until > self.tick);
                if !immune && self.disease.rng.chance(cold_permille, 1000) {
                    self.fall_ill(id, DiseaseKind::WinterFever);
                }
            }
        }
        self.request_doctor_jobs();
    }

    /// The colonist in a bed on a tile that a doctor would treat now.
    fn patient_in_bed(&self, x: u16, y: u16) -> Option<ColonistId> {
        self.colonists.iter().find_map(|(id, c)| {
            let in_bed = matches!(
                c.act,
                Some(NeedAct::Sleep {
                    bed: Some(b),
                    asleep: true
                }) if b == (x, y)
            );
            let p = self.disease.patients.get(&id);
            let sick = p.is_some_and(|p| !p.illnesses.is_empty());
            let treated = p.is_some_and(|p| p.treated_until > self.tick);
            (in_bed && (sick || c.wounded) && !treated).then_some(id)
        })
    }

    /// True while a doctor job at the tile has a patient and medicine exists.
    pub(crate) fn doctor_job_wanted(&self, x: u16, y: u16) -> bool {
        self.patient_in_bed(x, y).is_some() && self.medicine_stack().is_some()
    }

    fn medicine_stack(&self) -> Option<crate::item::ItemId> {
        let medicine = self.rules.medicine;
        self.items
            .iter()
            .find(|(_, i)| i.material == medicine && matches!(i.loc, ItemLoc::Tile(..)))
            .map(|(id, _)| id)
    }

    fn request_doctor_jobs(&mut self) {
        if self.medicine_stack().is_none() {
            return;
        }
        let beds: Vec<(u16, u16)> = self
            .colonists
            .iter()
            .filter_map(|(_, c)| match c.act {
                Some(NeedAct::Sleep {
                    bed: Some(b),
                    asleep: true,
                }) => Some(b),
                _ => None,
            })
            .collect();
        for (x, y) in beds {
            if self.patient_in_bed(x, y).is_some()
                && self
                    .jobs
                    .find_arg(JobKind::Mine, x, y, DOCTOR_ARG)
                    .is_none()
            {
                self.add_job_arg(JobKind::Mine, x, y, DOCTOR_ARG);
            }
        }
    }

    /// The doctor finishes a treatment (R-064).
    pub(crate) fn complete_doctor_work(&mut self, id: ColonistId, active: ActiveJob, job: &Job) {
        let (Some(patient), Some(stack)) =
            (self.patient_in_bed(job.x, job.y), self.medicine_stack())
        else {
            self.abort_job(id, AbortReason::JobGone, false);
            return;
        };
        self.items.take_one(stack);
        let r = self.rules.disease.clone();
        let level = u32::from(self.skill_level(id, Skill::Medicine).unwrap_or(0));
        let pct = (r.treat_base_pct + r.treat_per_level_pct * level).min(100);
        let success = self.disease.rng.chance(pct, 100);
        if success {
            let until = self.tick + u64::from(r.treat_days) * TICKS_PER_DAY;
            self.disease
                .patients
                .entry(patient)
                .or_default()
                .treated_until = until;
        }
        self.disease_events.push(DiseaseEvent::Treated {
            x: job.x,
            y: job.y,
            success,
        });
        self.finish_job_skill(id, active, job.x, job.y, Some(Skill::Medicine));
    }

    fn disease_day(&mut self) {
        let r = self.rules.disease.clone();
        let tick = self.tick;
        let grace = self.grace_over();
        let ids: Vec<ColonistId> = self.colonists.iter().map(|(id, _)| id).collect();
        self.disease.patients.retain(|id, _| ids.contains(id));
        // Severity changes and wound infections.
        for &id in &ids {
            let Some(c) = self.colonists.get(id) else {
                continue;
            };
            let (x, y) = c.tile();
            let wounded = c.wounded;
            let Some(mut p) = self.disease.patients.remove(&id) else {
                if wounded && grace {
                    self.wound_roll(id, &Patient::default(), &r);
                }
                continue;
            };
            let mut kept = Vec::new();
            for ill in &p.illnesses {
                let def = def_of(&r, ill.kind);
                let (_, delta) = self.daily_change(&p, ill.kind, def);
                let severity = i64::from(ill.severity) + i64::from(delta);
                if severity <= 0 {
                    if ill.kind == DiseaseKind::WinterFever {
                        p.immune_until = tick + u64::from(r.immune_days) * TICKS_PER_DAY;
                    }
                    self.disease_events.push(DiseaseEvent::Recovered {
                        kind: ill.kind,
                        x,
                        y,
                    });
                } else {
                    kept.push(Illness {
                        kind: ill.kind,
                        severity: u32::try_from(severity.min(100)).unwrap_or(100),
                    });
                }
            }
            p.illnesses = kept;
            self.disease.patients.insert(id, p);
            if wounded && grace {
                let snapshot = self.disease.patients.get(&id).cloned().unwrap_or_default();
                self.wound_roll(id, &snapshot, &r);
            }
        }
        // Winter fever spreads between colonists that sleep in the same room.
        let sleepers: Vec<(ColonistId, (u16, u16), bool, bool)> = ids
            .iter()
            .filter_map(|&id| {
                let p = self.disease.patients.get(&id)?;
                Some((
                    id,
                    p.slept_room?,
                    p.room_comfort,
                    p.has(DiseaseKind::WinterFever),
                ))
            })
            .collect();
        if grace {
            for &(target, room, _, _) in &sleepers {
                for &(source, source_room, comfort, sick) in &sleepers {
                    if !sick || source == target || source_room != room {
                        continue;
                    }
                    let pct = if comfort {
                        r.infect_comfort_pct
                    } else {
                        r.infect_pct
                    };
                    if self.disease.rng.chance(pct, 100) {
                        self.fall_ill(target, DiseaseKind::WinterFever);
                    }
                }
            }
        }
        // A new day: forget the night and drop the colonists with nothing to track.
        for p in self.disease.patients.values_mut() {
            p.slept_bed = false;
            p.slept_warm = false;
            p.slept_room = None;
            p.room_comfort = false;
        }
        self.disease.patients.retain(|_, p| !p.is_idle(tick));
    }

    fn wound_roll(&mut self, id: ColonistId, p: &Patient, r: &base::content::DiseaseRules) {
        if p.has(DiseaseKind::WoundInfection) {
            return;
        }
        let pct = if p.treated_until > self.tick {
            r.wound_pct_treated
        } else if p.slept_bed {
            r.wound_pct_bed
        } else {
            r.wound_pct_no_bed
        };
        if self.disease.rng.chance(pct, 100) {
            self.fall_ill(id, DiseaseKind::WoundInfection);
        }
    }

    // ---- sick rooms ----

    /// Mark the room that contains a tile as a sick room, or take the mark away (R-064).
    pub(crate) fn set_sick_room(&mut self, x: u16, y: u16, on: bool) {
        match self.room_key(x, y) {
            Some(key) => {
                if on {
                    self.disease.sick_rooms.insert(key);
                } else {
                    self.disease.sick_rooms.remove(&key);
                }
            }
            None => self.reject(Reject::UnknownTarget),
        }
    }

    /// Keep only the beds that fit the patient: a sick colonist takes a bed of a sick room if
    /// one is free. A healthy colonist leaves the sick rooms alone if another bed is free.
    pub(crate) fn sick_room_filter(
        &self,
        id: ColonistId,
        beds: Vec<(u16, u16)>,
    ) -> Vec<(u16, u16)> {
        if self.disease.sick_rooms.is_empty() {
            return beds;
        }
        let want_sick = self.is_sick(id);
        let fits: Vec<(u16, u16)> = beds
            .iter()
            .copied()
            .filter(|&(x, y)| self.is_sick_room(x, y) == want_sick)
            .collect();
        if fits.is_empty() { beds } else { fits }
    }

    /// True if the disease numbers of a saved state are valid.
    pub(crate) fn disease_state_ok(&self) -> bool {
        let inside = |t: &(u16, u16)| self.map.in_bounds(i32::from(t.0), i32::from(t.1));
        self.disease.sick_rooms.iter().all(inside)
            && self.disease.patients.values().all(|p| {
                p.illnesses.iter().all(|i| (1..=100).contains(&i.severity))
                    && p.slept_room.as_ref().is_none_or(inside)
            })
    }

    // ---- debug ----

    /// Give a colonist a disease now. For tests and tools. It ignores the grace days.
    pub fn debug_infect(&mut self, id: ColonistId, kind: DiseaseKind) -> bool {
        self.fall_ill(id, kind)
    }

    /// Set the severity of a disease of a colonist. For tests and tools.
    pub fn debug_set_severity(&mut self, id: ColonistId, kind: DiseaseKind, severity: u32) {
        if let Some(p) = self.disease.patients.get_mut(&id)
            && let Some(i) = p.illnesses.iter_mut().find(|i| i.kind == kind)
        {
            i.severity = severity.clamp(1, 100);
        }
    }

    /// Treat a colonist for a number of ticks without a doctor. For tests and tools.
    pub fn debug_set_treated(&mut self, id: ColonistId, ticks: u64) {
        self.disease.patients.entry(id).or_default().treated_until = self.tick + ticks;
    }

    /// Run the daily disease step now. For tests.
    pub fn debug_disease_day(&mut self) {
        self.disease_day();
    }
}
