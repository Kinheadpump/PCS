//! `tools soak`: a long run on each test preset with a seeded scripted player. It checks the
//! invariants of the sim, a save round trip, and the run state. A lost colony restarts with
//! the next seed.

use base::rng::Pcg32;
use sim::{Command, JobKind, RunState, Sim, save};
use std::time::Instant;
use worldgen::{EmbarkSummary, LocalMapData, generate_local};

const DEFAULT_TICKS: u64 = 1_000_000;
const PRESETS: [&str; 3] = ["temperate_forest", "cold_hills", "dry_plain"];
/// Ticks between two invariant checks.
const CHECK_EVERY: u64 = 600;
/// Ticks between two save round trips.
const SAVE_EVERY: u64 = 100_000;
/// Ticks between two player actions (one game day).
const ACT_EVERY: u64 = 2400;

/// What one preset run found.
struct Report {
    preset: &'static str,
    ticks: u64,
    colonies: u32,
    lost: u32,
    longest: u64,
    peak: u32,
    round_trips: u32,
    entries: usize,
    hash: u64,
    millis: u128,
}

fn parse(args: &[String]) -> Result<u64, String> {
    let usage = "usage: tools soak [--ticks <n>]";
    let mut ticks = DEFAULT_TICKS;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--ticks" => {
                let v = it.next().ok_or(usage)?;
                ticks = v.parse().map_err(|e| format!("bad ticks {v:?}: {e}"))?;
            }
            _ => return Err(usage.to_owned()),
        }
    }
    if ticks == 0 {
        return Err("ticks must be at least 1".to_owned());
    }
    Ok(ticks)
}

fn embark(name: &str) -> Result<EmbarkSummary, String> {
    super::preset(name)
}

fn start(data: &LocalMapData, mean: i16, seed: u64) -> Sim {
    let mut sim = Sim::new(data, seed, 0);
    let params = sim.climate_params();
    sim.set_climate(mean, params.amplitude, params.moisture);
    sim
}

/// The commands of the scripted player for one game day.
fn act(sim: &Sim, rng: &mut Pcg32, day: u64, out: &mut Vec<Command>) {
    let Some((cx, cy)) = sim.colonists().iter().next().map(|(_, c)| c.tile()) else {
        return;
    };
    let rules = sim.rules();
    let (w, h) = (sim.map().width() as i32, sim.map().height() as i32);
    let near = |rng: &mut Pcg32, span: u32| -> (u16, u16) {
        let dx = rng.below(span * 2 + 1) as i32 - span as i32;
        let dy = rng.below(span * 2 + 1) as i32 - span as i32;
        (
            (i32::from(cx) + dx).clamp(0, w - 1) as u16,
            (i32::from(cy) + dy).clamp(0, h - 1) as u16,
        )
    };
    let crops = rules.farm.crops.len() as u32;
    if day == 0 {
        if let (Some(kitchen), Some(log)) =
            (rules.building_index("kitchen"), rules.material_index("log"))
        {
            out.push(Command::PlaceBlueprint {
                building: kitchen,
                x: cx.saturating_add(3),
                y: cy,
                material: log,
            });
        }
        if crops > 0 {
            out.push(Command::SetFarmZone {
                x0: cx.saturating_add(5),
                y0: cy,
                x1: cx.saturating_add(9),
                y1: cy.saturating_add(4),
                crop: 0,
            });
        }
    }
    let actions = 2 + rng.below(3);
    for _ in 0..actions {
        let (x, y) = near(rng, 25);
        match rng.below(9) {
            0 => out.push(Command::Designate {
                kind: JobKind::Cut,
                x,
                y,
            }),
            1 => out.push(Command::Designate {
                kind: JobKind::Mine,
                x,
                y,
            }),
            2 => out.push(Command::Designate {
                kind: JobKind::Gather,
                x,
                y,
            }),
            3 => out.push(Command::Designate {
                kind: JobKind::Haul,
                x,
                y,
            }),
            4 if crops > 0 => {
                let (x1, y1) = near(rng, 25);
                out.push(Command::SetFarmZone {
                    x0: x,
                    y0: y,
                    x1: x.saturating_add(3).min(x1.max(x)),
                    y1: y.saturating_add(3).min(y1.max(y)),
                    crop: rng.below(crops) as u16,
                });
            }
            5 => {
                let names = ["hearth", "wall", "door", "roof", "workshop", "kitchen"];
                let name = names[rng.below(names.len() as u32) as usize];
                let material = if rng.below(2) == 0 { "log" } else { "granite" };
                if let (Some(building), Some(material)) =
                    (rules.building_index(name), rules.material_index(material))
                {
                    out.push(Command::PlaceBlueprint {
                        building,
                        x,
                        y,
                        material,
                    });
                }
            }
            6 => {
                let n = rules.recipes.len() as u32;
                if n > 0 {
                    out.push(Command::SetKeep {
                        x: cx.saturating_add(3),
                        y: cy,
                        recipe: rng.below(n) as u16,
                        target: 10 + rng.below(40),
                    });
                }
            }
            7 => out.push(Command::ClearFarmZone {
                x0: x,
                y0: y,
                x1: x.saturating_add(2),
                y1: y.saturating_add(2),
            }),
            _ => {}
        }
    }
}

#[allow(clippy::disallowed_methods)] // tools may read the clock
fn soak_preset(name: &'static str, ticks: u64) -> Result<Report, String> {
    let embark = embark(name)?;
    let data = generate_local(&embark);
    let mean = embark.mean_temperature;
    let started = Instant::now();
    let mut rng = Pcg32::from_seed(0x534f_414b ^ u64::from(mean as u16));
    let mut seed = 1u64;
    let mut sim = start(&data, mean, seed);
    let mut r = Report {
        preset: name,
        ticks: 0,
        colonies: 1,
        lost: 0,
        longest: 0,
        peak: 0,
        round_trips: 0,
        entries: 0,
        hash: 0,
        millis: 0,
    };
    let mut commands = Vec::new();
    for t in 0..ticks {
        commands.clear();
        if sim.tick().is_multiple_of(ACT_EVERY) {
            act(&sim, &mut rng, sim.tick() / ACT_EVERY, &mut commands);
        }
        sim.step(&commands);
        sim.drain_events();
        sim.drain_job_events();
        sim.drain_life_events();
        r.ticks = t + 1;
        let here = |msg: String| format!("{name}: tick {} (total {}): {msg}", sim.tick(), t + 1);
        if (t + 1).is_multiple_of(CHECK_EVERY) {
            sim.check_invariants().map_err(here)?;
        }
        if (t + 1).is_multiple_of(SAVE_EVERY) {
            let bytes = save::to_bytes(&sim);
            let loaded =
                save::from_bytes(&bytes).map_err(|e| here(format!("save did not load: {e:?}")))?;
            if loaded.state_hash() != sim.state_hash() {
                return Err(here("save round trip changed the state".to_owned()));
            }
            r.round_trips += 1;
        }
        r.peak = r.peak.max(sim.run_info().peak_population);
        if let RunState::Lost { tick } = sim.run_state() {
            sim.check_invariants().map_err(here)?;
            r.lost += 1;
            r.longest = r.longest.max(tick);
            r.entries += sim.chronicle().len();
            seed += 1;
            r.colonies += 1;
            sim = start(&data, mean, seed);
        }
    }
    sim.check_invariants()
        .map_err(|e| format!("{name}: end: {e}"))?;
    r.longest = r.longest.max(sim.tick());
    r.entries += sim.chronicle().len();
    r.hash = sim.state_hash();
    r.millis = started.elapsed().as_millis();
    Ok(r)
}

/// Run the subcommand.
pub fn run(args: &[String]) -> Result<String, String> {
    let ticks = parse(args)?;
    let mut out = String::new();
    for name in PRESETS {
        let r = soak_preset(name, ticks)?;
        out.push_str(&format!(
            "{}: ok, {} ticks, {} colonies ({} lost), longest {} ticks, peak {} colonists, {} save round trips, {} chronicle entries, hash {:016x}, {} ms\n",
            r.preset, r.ticks, r.colonies, r.lost, r.longest, r.peak, r.round_trips, r.entries, r.hash, r.millis
        ));
    }
    out.push_str("soak: all invariants held");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soak_runs_a_short_time_on_all_presets() {
        let out = run(&["--ticks".to_owned(), "3000".to_owned()]).unwrap_or_default();
        assert!(out.contains("all invariants held"), "{out}");
    }

    #[test]
    fn soak_rejects_bad_arguments() {
        assert!(run(&["--ticks".to_owned(), "x".to_owned()]).is_err());
        assert!(run(&["--ticks".to_owned(), "0".to_owned()]).is_err());
        assert!(run(&["--bad".to_owned()]).is_err());
    }
}
