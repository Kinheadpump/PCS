//! Content for needs, comfort, warmth, health, mood, and traits (`needs.ron`, `traits.ron`).
//!
//! The raw files hold string keys. The loader links them to material indices, food entries,
//! and memory entries, and checks every number. All numbers are integers.

use base::content::{ContentError, ContentIssue, MaterialDef};
use serde::Deserialize;

/// One mood memory in the file: a timed modifier.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryDef {
    /// String ID.
    pub key: String,
    /// Display name.
    pub name: String,
    /// Mood points.
    pub value: i16,
    /// How long the memory lasts, in game hours.
    pub hours: u32,
    /// True for a danger modifier (the Bold trait makes it smaller).
    pub danger: bool,
}

/// Raw `needs.ron`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNeeds {
    hunger_start: u32,
    hunger_max: u32,
    hunger_per_day: u32,
    hunger_eat_at: u32,
    eat_ticks: u32,
    starving_health_loss: u32,
    rest_start: u32,
    rest_max: u32,
    rest_per_awake_hour: u32,
    rest_sleep_at: u32,
    sleep_per_hour_bed: u32,
    sleep_per_hour_ground: u32,
    exhausted_speed_pct: i16,
    need_retry_ticks: u32,
    comfort_low: i16,
    comfort_high: i16,
    start_clothes: String,
    clothes: Vec<(String, i16)>,
    hazard_margin: i16,
    hazard_step: u32,
    hazard_recovery: u32,
    hazard_max: u32,
    hazard_limit: u32,
    hazard_health_loss: u32,
    health_max: u32,
    heal_in_bed: u32,
    heal_elsewhere: u32,
    wounded_health_loss: u32,
    wounded_bed_hours: u32,
    foods: Vec<(String, u32, String)>,
    start_food: Vec<(String, u32)>,
    mood_base: i16,
    mood_min: i16,
    mood_max: i16,
    mood_hungry: i16,
    mood_starving: i16,
    mood_tired: i16,
    mood_exhausted: i16,
    mood_cold: i16,
    mood_hot: i16,
    mood_wounded: i16,
    beauty_divisor: i32,
    beauty_mood_max: i16,
    memories: Vec<MemoryDef>,
    mood_high_from: i16,
    mood_high_speed_pct: i16,
    mood_low_below: i16,
    mood_low_speed_pct: i16,
    wander_chance_pct: u32,
    wander_hours: u32,
    wander_radius: u16,
    mood_break_below: i16,
    break_hours: u32,
    leave_after_hours: u32,
    alert_death_hours: u32,
    age_min: u8,
    age_max: u8,
    graveyard_max: u32,
}

/// A food: a material that a colonist can eat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FoodDef {
    /// Material index.
    pub material: u16,
    /// Hunger that one unit adds (whole units).
    pub nutrition: u32,
    /// Index of the mood memory that eating it makes.
    pub memory: u8,
}

/// Clothes: a material that a colonist can wear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClothesDef {
    /// Material index.
    pub material: u16,
    /// Tenths of a degree that the comfort band lower limit drops.
    pub insulation: i16,
}

/// Numbers for needs, comfort, warmth, health, and mood. Linked and checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeedRules {
    /// False switches the whole needs system off. Not in the file; only tests use it.
    pub enabled: bool,
    /// Hunger of a new colonist (R-011).
    pub hunger_start: u32,
    /// Largest hunger.
    pub hunger_max: u32,
    /// Hunger lost each game day.
    pub hunger_per_day: u32,
    /// A colonist with this hunger or less goes to eat.
    pub hunger_eat_at: u32,
    /// Ticks that eating one unit takes.
    pub eat_ticks: u32,
    /// Health lost each game hour while starving.
    pub starving_health_loss: u32,
    /// Rest of a new colonist (R-012).
    pub rest_start: u32,
    /// Largest rest. A sleeper wakes at this value.
    pub rest_max: u32,
    /// Rest lost each awake game hour.
    pub rest_per_awake_hour: u32,
    /// A colonist with this rest or less goes to sleep.
    pub rest_sleep_at: u32,
    /// Rest gained each hour of sleep in a bed.
    pub sleep_per_hour_bed: u32,
    /// Rest gained each hour of sleep on the ground.
    pub sleep_per_hour_ground: u32,
    /// Work speed percent points of an exhausted colonist.
    pub exhausted_speed_pct: i16,
    /// Ticks before a colonist looks again for food or a bed.
    pub need_retry_ticks: u32,
    /// Lower limit of the comfort band without clothes, tenths of a degree (R-001).
    pub comfort_low: i16,
    /// Upper limit of the comfort band, tenths of a degree.
    pub comfort_high: i16,
    /// Material that new colonists wear.
    pub start_clothes: u16,
    /// All clothes, in file order.
    pub clothes: Vec<ClothesDef>,
    /// Hypothermia or heatstroke grows when the temperature is this far outside the band
    /// (tenths of a degree, R-013).
    pub hazard_margin: i16,
    /// Growth of hypothermia or heatstroke each hour outside the margin.
    pub hazard_step: u32,
    /// Fall of hypothermia or heatstroke each hour inside the band.
    pub hazard_recovery: u32,
    /// Largest hypothermia or heatstroke.
    pub hazard_max: u32,
    /// Hypothermia or heatstroke above this value costs health.
    pub hazard_limit: u32,
    /// Health lost each hour above the limit.
    pub hazard_health_loss: u32,
    /// Largest health (R-016).
    pub health_max: u32,
    /// Health gained each hour without a condition, in a bed.
    pub heal_in_bed: u32,
    /// Health gained each hour without a condition, elsewhere.
    pub heal_elsewhere: u32,
    /// Health lost each hour by a wounded colonist.
    pub wounded_health_loss: u32,
    /// Hours of sleep in a bed that heal a wound.
    pub wounded_bed_hours: u32,
    /// All foods, best first.
    pub foods: Vec<FoodDef>,
    /// Food of a new colony: (material, units).
    pub start_food: Vec<(u16, u32)>,
    /// Mood without modifiers (R-014).
    pub mood_base: i16,
    /// Smallest mood.
    pub mood_min: i16,
    /// Largest mood.
    pub mood_max: i16,
    /// Mood modifier of a hungry colonist.
    pub mood_hungry: i16,
    /// Mood modifier of a starving colonist.
    pub mood_starving: i16,
    /// Mood modifier of a tired colonist.
    pub mood_tired: i16,
    /// Mood modifier of an exhausted colonist.
    pub mood_exhausted: i16,
    /// Mood modifier below the lower limit of the comfort band (R-013).
    pub mood_cold: i16,
    /// Mood modifier above the upper limit of the comfort band.
    pub mood_hot: i16,
    /// Mood modifier of a wounded colonist.
    pub mood_wounded: i16,
    /// Room beauty per tile divided by this gives mood points.
    pub beauty_divisor: i32,
    /// Largest size of the room beauty modifier, up or down.
    pub beauty_mood_max: i16,
    /// Mood memories, in file order.
    pub memories: Vec<MemoryDef>,
    /// Mood from this value gives the work speed bonus (R-015).
    pub mood_high_from: i16,
    /// Work speed percent points at high mood.
    pub mood_high_speed_pct: i16,
    /// Mood below this value gives the low mood effects.
    pub mood_low_below: i16,
    /// Work speed percent points at low mood.
    pub mood_low_speed_pct: i16,
    /// Chance in percent each hour that a low mood colonist walks around.
    pub wander_chance_pct: u32,
    /// Hours of a walk-around.
    pub wander_hours: u32,
    /// Largest distance of a walk-around, in tiles.
    pub wander_radius: u16,
    /// Mood below this value is a breakdown.
    pub mood_break_below: i16,
    /// Hours that a breakdown stops work.
    pub break_hours: u32,
    /// Hours of breakdown mood after which the colonist leaves.
    pub leave_after_hours: u32,
    /// Hours after a death in which the death alert shows.
    pub alert_death_hours: u32,
    /// Youngest age of a new colonist.
    pub age_min: u8,
    /// Oldest age of a new colonist.
    pub age_max: u8,
    /// Most graves that the sim keeps.
    pub graveyard_max: u32,
}

impl NeedRules {
    /// Rules that do nothing. For the empty rules that `SimRules` uses if the content is
    /// invalid.
    pub fn empty() -> Self {
        Self {
            enabled: false,
            hunger_start: 100,
            hunger_max: 100,
            hunger_per_day: 0,
            hunger_eat_at: 0,
            eat_ticks: 1,
            starving_health_loss: 0,
            rest_start: 100,
            rest_max: 100,
            rest_per_awake_hour: 0,
            rest_sleep_at: 0,
            sleep_per_hour_bed: 0,
            sleep_per_hour_ground: 0,
            exhausted_speed_pct: 0,
            need_retry_ticks: 50,
            comfort_low: 100,
            comfort_high: 260,
            start_clothes: u16::MAX,
            clothes: Vec::new(),
            hazard_margin: 100,
            hazard_step: 0,
            hazard_recovery: 0,
            hazard_max: 100,
            hazard_limit: 50,
            hazard_health_loss: 0,
            health_max: 100,
            heal_in_bed: 0,
            heal_elsewhere: 0,
            wounded_health_loss: 0,
            wounded_bed_hours: 8,
            foods: Vec::new(),
            start_food: Vec::new(),
            mood_base: 50,
            mood_min: 0,
            mood_max: 100,
            mood_hungry: 0,
            mood_starving: 0,
            mood_tired: 0,
            mood_exhausted: 0,
            mood_cold: 0,
            mood_hot: 0,
            mood_wounded: 0,
            beauty_divisor: 1,
            beauty_mood_max: 0,
            memories: Vec::new(),
            mood_high_from: 70,
            mood_high_speed_pct: 0,
            mood_low_below: 30,
            mood_low_speed_pct: 0,
            wander_chance_pct: 0,
            wander_hours: 1,
            wander_radius: 1,
            mood_break_below: 15,
            break_hours: 4,
            leave_after_hours: 120,
            alert_death_hours: 24,
            age_min: 18,
            age_max: 45,
            graveyard_max: 100,
        }
    }

    /// The food entry of a material, if it is a food.
    pub fn food(&self, material: u16) -> Option<&FoodDef> {
        self.foods.iter().find(|f| f.material == material)
    }

    /// The clothes entry of a material, if it is clothes.
    pub fn clothes_def(&self, material: u16) -> Option<&ClothesDef> {
        self.clothes.iter().find(|c| c.material == material)
    }

    /// The index of a memory by its string ID.
    pub fn memory_index(&self, key: &str) -> Option<u8> {
        self.memories
            .iter()
            .position(|m| m.key == key)
            .and_then(|i| u8::try_from(i).ok())
    }
}

/// Load, check, and link `needs.ron`. `materials` are the linked materials.
pub fn load_need_rules(
    file_name: &str,
    text: &str,
    materials: &[MaterialDef],
) -> Result<NeedRules, ContentError> {
    let fail = |message: String| ContentError {
        file: file_name.to_string(),
        issues: vec![ContentIssue {
            entry: None,
            message,
        }],
    };
    let raw: RawNeeds = ron::from_str(text).map_err(|e| fail(format!("parse error: {e}")))?;
    let mut issues = Vec::new();
    let mut bad = |entry: &str, message: &str| {
        issues.push(ContentIssue {
            entry: Some(entry.to_string()),
            message: message.to_string(),
        });
    };
    let material = |key: &str| {
        materials
            .iter()
            .position(|m| m.key == key)
            .and_then(|i| u16::try_from(i).ok())
    };
    let link = |what: &str, key: &str, bad: &mut dyn FnMut(&str, &str)| -> u16 {
        material(key).unwrap_or_else(|| {
            bad(key, &format!("{what} uses an unknown material"));
            u16::MAX
        })
    };
    let start_clothes = link("start_clothes", &raw.start_clothes, &mut bad);
    let clothes: Vec<ClothesDef> = raw
        .clothes
        .iter()
        .map(|(key, insulation)| ClothesDef {
            material: link("clothes", key, &mut bad),
            insulation: *insulation,
        })
        .collect();
    if !clothes.iter().any(|c| c.material == start_clothes) {
        bad(
            &raw.start_clothes,
            "start_clothes must be in the clothes list",
        );
    }
    if clothes.iter().any(|c| !(0..=400).contains(&c.insulation)) {
        bad("clothes", "insulation must be from 0 to 400");
    }
    let mut foods = Vec::new();
    for (key, nutrition, memory) in &raw.foods {
        let m = link("foods", key, &mut bad);
        let mem = raw
            .memories
            .iter()
            .position(|d| &d.key == memory)
            .and_then(|i| u8::try_from(i).ok());
        let Some(mem) = mem else {
            bad(key, &format!("unknown memory '{memory}'"));
            continue;
        };
        if *nutrition == 0 || *nutrition > raw.hunger_max {
            bad(key, "nutrition must be from 1 to hunger_max");
        }
        foods.push(FoodDef {
            material: m,
            nutrition: *nutrition,
            memory: mem,
        });
    }
    let start_food: Vec<(u16, u32)> = raw
        .start_food
        .iter()
        .map(|(key, n)| (link("start_food", key, &mut bad), *n))
        .collect();
    if start_food
        .iter()
        .any(|(m, n)| *n > 10_000 || !foods.iter().any(|f| f.material == *m))
    {
        bad("start_food", "must be foods with at most 10000 units");
    }
    if raw.memories.len() > 200 {
        bad("memories", "at most 200 memories");
    }
    for (i, a) in raw.memories.iter().enumerate() {
        if raw.memories[..i].iter().any(|b| b.key == a.key) {
            bad(&a.key, "duplicate memory key");
        }
        if a.hours == 0 || a.hours > 10_000 || !(-100..=100).contains(&a.value) {
            bad(&a.key, "hours must be from 1 to 10000, value within 100");
        }
    }
    let pct_ok = |v: u32| v <= 100;
    if raw.hunger_max == 0 || raw.hunger_max > 1000 || raw.hunger_start > raw.hunger_max {
        bad("hunger_max", "hunger values must fit 1 to 1000");
    }
    if raw.rest_max == 0 || raw.rest_max > 1000 || raw.rest_start > raw.rest_max {
        bad("rest_max", "rest values must fit 1 to 1000");
    }
    if raw.hunger_per_day > 1000
        || raw.rest_per_awake_hour > 1000
        || raw.sleep_per_hour_bed > 1000
        || raw.sleep_per_hour_ground > 1000
        || raw.hazard_step > 1000
        || raw.hazard_recovery > 1000
        || raw.hazard_health_loss > 1000
        || raw.starving_health_loss > 1000
        || raw.wounded_health_loss > 1000
        || raw.heal_in_bed > 1000
        || raw.heal_elsewhere > 1000
    {
        bad("rates", "rates must be at most 1000");
    }
    if raw.eat_ticks == 0 || raw.eat_ticks > 10_000 || raw.need_retry_ticks == 0 {
        bad("eat_ticks", "must be from 1 to 10000");
    }
    if raw.health_max == 0 || raw.health_max > 1000 {
        bad("health_max", "must be from 1 to 1000");
    }
    if raw.hazard_max == 0 || raw.hazard_max > 1000 {
        bad("hazard_max", "must be from 1 to 1000");
    }
    if !(-300..=500).contains(&raw.comfort_low)
        || !(0..=600).contains(&raw.comfort_high)
        || raw.comfort_low >= raw.comfort_high
        || raw.hazard_margin < 0
    {
        bad("comfort_low", "the comfort band is out of range");
    }
    if raw.mood_min >= raw.mood_max || !(raw.mood_min..=raw.mood_max).contains(&raw.mood_base) {
        bad(
            "mood_base",
            "mood_min < mood_max, and mood_base between them",
        );
    }
    if raw.mood_break_below > raw.mood_low_below || raw.mood_low_below > raw.mood_high_from {
        bad("mood_low_below", "break < low < high must hold");
    }
    if !pct_ok(raw.wander_chance_pct) || raw.wander_radius == 0 {
        bad("wander_chance_pct", "chance up to 100, radius 1 or more");
    }
    if raw.beauty_divisor <= 0 || raw.beauty_mood_max < 0 {
        bad("beauty_divisor", "divisor above 0, cap 0 or more");
    }
    if raw.age_min == 0 || raw.age_min > raw.age_max {
        bad("age_min", "age_min from 1 up to age_max");
    }
    if raw.graveyard_max == 0 || raw.graveyard_max > 100_000 {
        bad("graveyard_max", "must be from 1 to 100000");
    }
    if raw.break_hours == 0
        || raw.wander_hours == 0
        || raw.leave_after_hours == 0
        || raw.wounded_bed_hours == 0
    {
        bad("break_hours", "hour counts must be 1 or more");
    }
    if !issues.is_empty() {
        return Err(ContentError {
            file: file_name.to_string(),
            issues,
        });
    }
    Ok(NeedRules {
        enabled: true,
        hunger_start: raw.hunger_start,
        hunger_max: raw.hunger_max,
        hunger_per_day: raw.hunger_per_day,
        hunger_eat_at: raw.hunger_eat_at,
        eat_ticks: raw.eat_ticks,
        starving_health_loss: raw.starving_health_loss,
        rest_start: raw.rest_start,
        rest_max: raw.rest_max,
        rest_per_awake_hour: raw.rest_per_awake_hour,
        rest_sleep_at: raw.rest_sleep_at,
        sleep_per_hour_bed: raw.sleep_per_hour_bed,
        sleep_per_hour_ground: raw.sleep_per_hour_ground,
        exhausted_speed_pct: raw.exhausted_speed_pct,
        need_retry_ticks: raw.need_retry_ticks,
        comfort_low: raw.comfort_low,
        comfort_high: raw.comfort_high,
        start_clothes,
        clothes,
        hazard_margin: raw.hazard_margin,
        hazard_step: raw.hazard_step,
        hazard_recovery: raw.hazard_recovery,
        hazard_max: raw.hazard_max,
        hazard_limit: raw.hazard_limit,
        hazard_health_loss: raw.hazard_health_loss,
        health_max: raw.health_max,
        heal_in_bed: raw.heal_in_bed,
        heal_elsewhere: raw.heal_elsewhere,
        wounded_health_loss: raw.wounded_health_loss,
        wounded_bed_hours: raw.wounded_bed_hours,
        foods,
        start_food,
        mood_base: raw.mood_base,
        mood_min: raw.mood_min,
        mood_max: raw.mood_max,
        mood_hungry: raw.mood_hungry,
        mood_starving: raw.mood_starving,
        mood_tired: raw.mood_tired,
        mood_exhausted: raw.mood_exhausted,
        mood_cold: raw.mood_cold,
        mood_hot: raw.mood_hot,
        mood_wounded: raw.mood_wounded,
        beauty_divisor: raw.beauty_divisor,
        beauty_mood_max: raw.beauty_mood_max,
        memories: raw.memories,
        mood_high_from: raw.mood_high_from,
        mood_high_speed_pct: raw.mood_high_speed_pct,
        mood_low_below: raw.mood_low_below,
        mood_low_speed_pct: raw.mood_low_speed_pct,
        wander_chance_pct: raw.wander_chance_pct,
        wander_hours: raw.wander_hours,
        wander_radius: raw.wander_radius,
        mood_break_below: raw.mood_break_below,
        break_hours: raw.break_hours,
        leave_after_hours: raw.leave_after_hours,
        alert_death_hours: raw.alert_death_hours,
        age_min: raw.age_min,
        age_max: raw.age_max,
        graveyard_max: raw.graveyard_max,
    })
}

// ---- traits ----

fn neutral_pct() -> u16 {
    100
}

/// A trait with its numbers. A neutral field changes nothing.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraitDef {
    /// String ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Tenths of a degree added to the lower limit of the comfort band.
    #[serde(default)]
    pub comfort_low_delta: i16,
    /// Hunger loss in percent of normal.
    #[serde(default = "neutral_pct")]
    pub hunger_rate_pct: u16,
    /// Size of the danger mood modifiers in percent of normal.
    #[serde(default = "neutral_pct")]
    pub danger_mood_pct: u16,
    /// Percent points added to the work speed.
    #[serde(default)]
    pub work_speed_pct: i16,
    /// Carry capacity in percent of normal.
    #[serde(default = "neutral_pct")]
    pub carry_pct: u16,
}

/// Load and check `traits.ron`.
pub fn load_traits(file_name: &str, text: &str) -> Result<Vec<TraitDef>, ContentError> {
    let list: Vec<TraitDef> = ron::from_str(text).map_err(|e| ContentError {
        file: file_name.to_string(),
        issues: vec![ContentIssue {
            entry: None,
            message: format!("parse error: {e}"),
        }],
    })?;
    let mut issues = Vec::new();
    if list.len() > 250 {
        issues.push(ContentIssue {
            entry: None,
            message: "at most 250 traits".to_string(),
        });
    }
    for (i, t) in list.iter().enumerate() {
        let mut bad = |message: &str| {
            issues.push(ContentIssue {
                entry: Some(t.id.clone()),
                message: message.to_string(),
            });
        };
        if list[..i].iter().any(|o| o.id == t.id) {
            bad("duplicate trait id");
        }
        if !(-300..=300).contains(&t.comfort_low_delta)
            || t.hunger_rate_pct > 1000
            || t.danger_mood_pct > 1000
            || t.carry_pct > 1000
            || !(-100..=100).contains(&t.work_speed_pct)
        {
            bad("a number is out of range");
        }
    }
    if issues.is_empty() {
        Ok(list)
    } else {
        Err(ContentError {
            file: file_name.to_string(),
            issues,
        })
    }
}

impl NeedRules {
    /// Give a new colonist the start values of the needs.
    pub fn init_colonist(&self, c: &mut crate::Colonist) {
        let milli = |v: u32| i32::try_from(v).unwrap_or(i32::MAX).saturating_mul(1000);
        c.hunger = milli(self.hunger_start);
        c.rest = milli(self.rest_start);
        c.health = i32::try_from(self.health_max).unwrap_or(i32::MAX);
        c.clothes = (self.start_clothes != u16::MAX).then_some(self.start_clothes);
    }
}
