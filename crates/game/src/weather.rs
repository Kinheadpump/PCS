//! The display of the climate: temperature texts, the forecast, the alerts, the temperature
//! overlay (R-002, R-003, R-022), and frozen water (R-004). Everything reads `&Sim`. The pure
//! parts (texts, colors, chips) have tests.

use crate::cells::icons as ic;
use crate::font::DEGREE_SIGN;
use crate::overlay::{Frame, screen_rect, thickness};
use crate::palette::Rgb;
use crate::render::color as rgb_color;
use macroquad::prelude::{draw_line, draw_rectangle};
use sim::{AlertKind, Sim, TempPlace};

/// Coldest temperature of the overlay scale, in tenths of a degree.
pub const SCALE_MIN: i32 = -150;
/// Warmest temperature of the overlay scale, in tenths of a degree.
pub const SCALE_MAX: i32 = 450;
/// Stops of the overlay scale: temperature in tenths of a degree and color. Cold is blue, warm
/// is red.
const STOPS: [(i32, Rgb); 7] = [
    (SCALE_MIN, [44, 70, 200]),
    (-50, [66, 140, 232]),
    (50, [104, 204, 232]),
    (150, [150, 222, 150]),
    (250, [240, 222, 96]),
    (350, [240, 130, 64]),
    (SCALE_MAX, [214, 48, 52]),
];
/// Opacity of the overlay tint outdoors and in rooms.
const OUTDOOR_ALPHA: u8 = 84;
const ROOM_ALPHA: u8 = 128;

/// Color of ice.
const ICE: Rgb = [196, 228, 242];
/// Color of the cracks and the shore line of ice.
const ICE_DARK: Rgb = [128, 172, 204];
/// Color of the sheen of ice.
const ICE_LIGHT: Rgb = [244, 250, 255];
/// Color of the alert chip of a cold period, of low fuel, and of an empty hearth.
pub const COLD_TINT: Rgb = [150, 206, 255];
pub const FUEL_TINT: Rgb = [255, 176, 72];
pub const OUT_TINT: Rgb = [244, 96, 88];

/// A temperature in tenths of a degree as a number with one decimal, for example `-3.5`.
pub fn tenths_text(tenths: i32) -> String {
    let a = tenths.unsigned_abs();
    let sign = if tenths < 0 { "-" } else { "" };
    format!("{sign}{}.{}", a / 10, a % 10)
}

/// The same with a sign for a part of a sum, for example `+11.0` or `-8.0`.
pub fn signed_text(tenths: i32) -> String {
    if tenths >= 0 {
        format!("+{}", tenths_text(tenths))
    } else {
        tenths_text(tenths)
    }
}

/// A temperature with its unit, for example `-3.5 C` with a degree sign.
pub fn temp_text(tenths: i32) -> String {
    format!("{}{DEGREE_SIGN}C", tenths_text(tenths))
}

/// The color of a temperature on the overlay scale.
pub fn temp_color(tenths: i32) -> Rgb {
    let t = tenths.clamp(SCALE_MIN, SCALE_MAX);
    for pair in STOPS.windows(2) {
        let ((t0, c0), (t1, c1)) = (pair[0], pair[1]);
        if t <= t1 {
            let span = t1 - t0;
            let at = t - t0;
            return std::array::from_fn(|i| {
                let a = i32::from(c0[i]);
                let b = i32::from(c1[i]);
                (a + (b - a) * at / span) as u8
            });
        }
    }
    STOPS[STOPS.len() - 1].1
}

/// Position of a temperature on the scale, from 0 (coldest) to 1000 (warmest).
pub fn scale_permille(tenths: i32) -> i32 {
    (tenths.clamp(SCALE_MIN, SCALE_MAX) - SCALE_MIN) * 1000 / (SCALE_MAX - SCALE_MIN)
}

/// The text of the temperature readout in the top bar.
pub fn top_text(sim: &Sim) -> String {
    temp_text(i32::from(sim.outdoor_temp()))
}

/// The tooltip of the readout: the outdoor temperature with its named parts, and the forecast of
/// the next three days.
pub fn forecast_tip(sim: &Sim) -> String {
    let p = sim.outdoor_parts();
    let mut lines = vec![
        format!("{} outdoors", temp_text(p.total())),
        format!(
            "{} mean  {} season",
            signed_text(p.mean),
            signed_text(p.season)
        ),
        format!(
            "{} day  {} weather",
            signed_text(p.daily),
            signed_text(p.weather)
        ),
    ];
    for (i, day) in sim.forecast().iter().enumerate() {
        let name = match i {
            0 => "Tomorrow".to_owned(),
            n => format!("In {} days", n + 1),
        };
        let rain = if day.rain { "  rain" } else { "" };
        lines.push(format!(
            "{name}: {}{rain}",
            temp_text(i32::from(day.mean_temp))
        ));
    }
    lines.join("\n")
}

/// One alert of the strip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlertChip {
    /// Icon cell.
    pub icon: usize,
    /// Short text.
    pub text: String,
    /// Color of the icon.
    pub tint: Rgb,
    /// The tile to center the camera on when the player clicks the chip.
    pub target: Option<(u16, u16)>,
}

/// Most chips in the strip. More alerts show as a count.
pub const MAX_CHIPS: usize = 3;

/// The chips of the alert strip. The cold period comes first, then the hearths. A click goes to
/// the hearth of the alert, or to the first hearth for the cold period.
pub fn alert_chips(sim: &Sim) -> Vec<AlertChip> {
    let first_hearth = sim.hearth_tiles().first().copied();
    let mut chips: Vec<AlertChip> = sim
        .alerts()
        .iter()
        .map(|alert| match alert.kind {
            AlertKind::ColdPeriod { days_ahead, .. } => AlertChip {
                icon: ic::SNOWFLAKE,
                text: if days_ahead == 0 {
                    "Cold".to_owned()
                } else {
                    format!("Cold in {days_ahead}d")
                },
                tint: COLD_TINT,
                target: first_hearth,
            },
            AlertKind::LowFuel { x, y, hours } => AlertChip {
                icon: ic::FLAME,
                text: if hours == 0 {
                    "No fuel".to_owned()
                } else {
                    format!("Fuel {hours}h")
                },
                tint: if hours == 0 { OUT_TINT } else { FUEL_TINT },
                target: Some((x, y)),
            },
            AlertKind::Hunger { x, y, starving, .. } => AlertChip {
                icon: ic::FLAME,
                text: if starving { "Starving" } else { "Hungry" }.to_owned(),
                tint: OUT_TINT,
                target: Some((x, y)),
            },
            AlertKind::Hypothermia { x, y, .. } => AlertChip {
                icon: ic::SNOWFLAKE,
                text: "Hypothermia".to_owned(),
                tint: COLD_TINT,
                target: Some((x, y)),
            },
            AlertKind::Heatstroke { x, y, .. } => AlertChip {
                icon: ic::FLAME,
                text: "Heatstroke".to_owned(),
                tint: OUT_TINT,
                target: Some((x, y)),
            },
            AlertKind::LowMood { x, y, .. } => AlertChip {
                icon: ic::FLAME,
                text: "Low mood".to_owned(),
                tint: FUEL_TINT,
                target: Some((x, y)),
            },
            AlertKind::Death { x, y, .. } => AlertChip {
                icon: ic::SNOWFLAKE,
                text: "Death".to_owned(),
                tint: OUT_TINT,
                target: Some((x, y)),
            },
        })
        .collect();
    if chips.len() > MAX_CHIPS {
        let more = chips.len() - (MAX_CHIPS - 1);
        chips.truncate(MAX_CHIPS - 1);
        chips.push(AlertChip {
            icon: ic::FLAME,
            text: format!("+{more}"),
            tint: FUEL_TINT,
            target: None,
        });
    }
    chips
}

/// The text of the temperature of a tile for the tile panel.
pub fn place_name(place: TempPlace) -> &'static str {
    match place {
        TempPlace::Outdoor => "Outdoors",
        TempPlace::Room => "Room",
        TempPlace::RockRoom => "Rock room",
    }
}

// ---- drawing ----

/// Draw the temperature overlay: a tint over the map for the outdoor temperature, and a stronger
/// tint over each room for its own temperature.
pub fn draw_temperature(f: &Frame) {
    let Some(v) = f.visible() else { return };
    let outdoor = i32::from(f.sim.outdoor_temp());
    let (x, y, w, h) = screen_rect(
        f.camera,
        f.view,
        v.min_x,
        v.min_y,
        v.max_x - v.min_x + 1,
        v.max_y - v.min_y + 1,
    );
    draw_rectangle(x, y, w, h, rgb_color(temp_color(outdoor), OUTDOOR_ALPHA));
    for room in f.sim.rooms().iter() {
        let Some(&(rx, ry)) = room.tiles.first() else {
            continue;
        };
        let Some(t) = f.sim.room_temp(rx, ry) else {
            continue;
        };
        let color = rgb_color(temp_color(i32::from(t)), ROOM_ALPHA);
        for &(tx, ty) in &room.tiles {
            if f.is_visible(&v, tx, ty) {
                let r = f.rect(tx, ty);
                draw_rectangle(r.0, r.1, r.2, r.3, color);
            }
        }
    }
}

/// A number from a tile position, the same on every run.
pub fn tile_hash(x: u16, y: u16) -> u32 {
    let mut h = u32::from(x).wrapping_mul(0x9E37_79B1) ^ u32::from(y).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

/// Draw the ice on the shallow water: a pale sheet, a line at the shore, cracks, and sheen.
pub fn draw_ice(f: &Frame) {
    if !f.sim.ice() {
        return;
    }
    let Some(v) = f.visible() else { return };
    let sheet = rgb_color(ICE, 214);
    let edge = rgb_color(ICE_DARK, 200);
    let crack = rgb_color(ICE_DARK, 170);
    let sheen = rgb_color(ICE_LIGHT, 230);
    let ice_at = |x: i32, y: i32| {
        u16::try_from(x)
            .ok()
            .zip(u16::try_from(y).ok())
            .is_some_and(|(x, y)| f.sim.ice_at(x, y))
    };
    for ty in v.min_y..=v.max_y {
        for tx in v.min_x..=v.max_x {
            if !ice_at(tx, ty) {
                continue;
            }
            let (Ok(x), Ok(y)) = (u16::try_from(tx), u16::try_from(ty)) else {
                continue;
            };
            let (rx, ry, rw, rh) = f.rect(x, y);
            draw_rectangle(rx, ry, rw, rh, sheet);
            let t = thickness(f.camera.zoom);
            if !ice_at(tx, ty - 1) {
                draw_rectangle(rx, ry, rw, t, edge);
            }
            if !ice_at(tx, ty + 1) {
                draw_rectangle(rx, ry + rh - t, rw, t, edge);
            }
            if !ice_at(tx - 1, ty) {
                draw_rectangle(rx, ry, t, rh, edge);
            }
            if !ice_at(tx + 1, ty) {
                draw_rectangle(rx + rw - t, ry, t, rh, edge);
            }
            let h = tile_hash(x, y);
            if h.is_multiple_of(3) {
                // A crack: two short strokes that meet at a point inside the tile.
                let p = |n: u32| (n % 5 + 3) as f32 / 16.0;
                let (ax, ay) = (rx + rw * p(h >> 3), ry + rh * p(h >> 6));
                let (bx, by) = (rx + rw * p(h >> 9), ry + rh * (0.5 + p(h >> 12) * 0.4));
                let (cx, cy) = (rx + rw * (0.5 + p(h >> 15) * 0.4), ry + rh * p(h >> 18));
                draw_line(ax, ay, bx, by, t, crack);
                draw_line(ax, ay, cx, cy, t, crack);
            }
            if h % 5 == 1 {
                let (sx, sy) = (
                    rx + rw * ((h >> 4) % 8 + 2) as f32 / 16.0,
                    ry + rh * ((h >> 8) % 8 + 2) as f32 / 16.0,
                );
                draw_rectangle(sx, sy, 2.0 * t, t, sheen);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::scenario::sim_from_ascii;

    #[test]
    fn temperatures_read_with_one_decimal_and_a_sign() {
        assert_eq!(tenths_text(-35), "-3.5");
        assert_eq!(tenths_text(-5), "-0.5");
        assert_eq!(tenths_text(0), "0.0");
        assert_eq!(tenths_text(110), "11.0");
        assert_eq!(signed_text(110), "+11.0");
        assert_eq!(signed_text(-80), "-8.0");
        assert_eq!(temp_text(-35), format!("-3.5{DEGREE_SIGN}C"));
    }

    #[test]
    fn the_scale_goes_from_blue_to_red_and_clamps() {
        let cold = temp_color(-500);
        let warm = temp_color(900);
        assert_eq!(cold, STOPS[0].1);
        assert_eq!(warm, STOPS[STOPS.len() - 1].1);
        assert!(cold[2] > cold[0], "cold is blue");
        assert!(warm[0] > warm[2], "warm is red");
        assert_eq!(temp_color(50), STOPS[2].1);
        assert_eq!(scale_permille(SCALE_MIN), 0);
        assert_eq!(scale_permille(SCALE_MAX), 1000);
        assert!(scale_permille(100) > scale_permille(0));
    }

    #[test]
    fn the_color_changes_step_by_step_between_the_stops() {
        let mut last = temp_color(SCALE_MIN);
        for t in SCALE_MIN..=SCALE_MAX {
            let c = temp_color(t);
            for i in 0..3 {
                assert!(c[i].abs_diff(last[i]) <= 4, "jump at {t}");
            }
            last = c;
        }
    }

    #[test]
    fn the_tip_lists_the_parts_and_three_forecast_days() {
        let sim = sim_from_ascii("c", 1).unwrap();
        let tip = forecast_tip(&sim);
        let lines: Vec<&str> = tip.lines().collect();
        assert_eq!(lines.len(), 6, "{tip}");
        assert!(lines[0].ends_with("outdoors"));
        assert!(lines[1].contains("mean") && lines[1].contains("season"));
        assert!(lines[2].contains("day") && lines[2].contains("weather"));
        assert!(lines[3].starts_with("Tomorrow"));
        assert!(lines[4].starts_with("In 2 days"));
        assert!(lines[5].starts_with("In 3 days"));
        let p = sim.outdoor_parts();
        assert_eq!(
            p.mean + p.season + p.daily + p.weather,
            i32::from(sim.outdoor_temp())
        );
    }

    #[test]
    fn a_cold_colony_with_a_dead_hearth_shows_chips_that_jump_to_the_hearth() {
        let mut sim = sim_from_ascii("c....", 1).unwrap();
        let hearth = sim.rules().building_index("hearth").unwrap();
        let granite = sim.rules().material_index("granite").unwrap();
        assert!(sim.debug_build(hearth, 3, 0, granite));
        assert!(sim.debug_set_hearth_fuel(3, 0, 0));
        let chips = alert_chips(&sim);
        let fuel = chips.iter().find(|c| c.icon == ic::FLAME).unwrap();
        assert_eq!(fuel.text, "No fuel");
        assert_eq!(fuel.target, Some((3, 0)));
        assert_eq!(fuel.tint, OUT_TINT);
        assert!(sim.debug_set_hearth_fuel(3, 0, 2));
        let chips = alert_chips(&sim);
        assert_eq!(
            chips.iter().find(|c| c.icon == ic::FLAME).unwrap().text,
            "Fuel 2h"
        );
    }

    #[test]
    fn the_strip_holds_at_most_three_chips() {
        let mut sim = sim_from_ascii("c.........", 1).unwrap();
        let hearth = sim.rules().building_index("hearth").unwrap();
        let granite = sim.rules().material_index("granite").unwrap();
        for x in 2..8 {
            assert!(sim.debug_build(hearth, x, 0, granite));
            assert!(sim.debug_set_hearth_fuel(x, 0, 0));
        }
        let chips = alert_chips(&sim);
        assert_eq!(chips.len(), MAX_CHIPS);
        assert_eq!(chips[MAX_CHIPS - 1].target, None);
        assert!(chips[MAX_CHIPS - 1].text.starts_with('+'));
    }

    #[test]
    fn the_tile_hash_is_repeatable_and_varies() {
        assert_eq!(tile_hash(3, 9), tile_hash(3, 9));
        let distinct: std::collections::BTreeSet<u32> =
            (0..8u16).map(|x| tile_hash(x, 2)).collect();
        assert_eq!(distinct.len(), 8);
    }

    #[test]
    fn places_have_names() {
        assert_eq!(place_name(TempPlace::Outdoor), "Outdoors");
        assert_ne!(place_name(TempPlace::Room), place_name(TempPlace::RockRoom));
    }
}
