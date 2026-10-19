//! `tools bench`: headless 300-colonist random-walk scenario (P-02).

use base::rng::Pcg32;
use sim::{Command, LocalMap, Regions, Sim};
use std::io::Write;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use worldgen::{EmbarkSummary, generate_local};

const COLONISTS: usize = 300;
const DEFAULT_TICKS: u32 = 2000;
/// P-02 limit in milliseconds.
const LIMIT_MS: &str = "3";
const CSV_PATH: &str = "bench/results.csv";

struct Options {
    ticks: u32,
    csv: bool,
    machine: String,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let usage = "usage: tools bench [--ticks <n>] [--no-csv] [--machine <text>]";
    let mut o = Options {
        ticks: DEFAULT_TICKS,
        csv: true,
        machine: "dev PC (CPU not recorded)".to_owned(),
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--ticks" => {
                let v = it.next().ok_or(usage)?;
                o.ticks = v.parse().map_err(|e| format!("bad ticks {v:?}: {e}"))?;
            }
            "--no-csv" => o.csv = false,
            "--machine" => o.machine = it.next().ok_or(usage)?.clone(),
            _ => return Err(usage.to_owned()),
        }
    }
    if o.ticks == 0 {
        return Err("ticks must be at least 1".to_owned());
    }
    Ok(o)
}

/// Passable tiles that are in the region of the map center.
fn region_tiles(map: &LocalMap, regions: &Regions) -> Vec<(u16, u16)> {
    let (w, h) = (map.width() as i32, map.height() as i32);
    let mut region = 0;
    // The first passable tile in rings from the center fixes the region.
    'rings: for ring in 0..w.max(h) {
        for y in h / 2 - ring..=h / 2 + ring {
            for x in w / 2 - ring..=w / 2 + ring {
                if map.passable(x, y) {
                    region = regions.id_at(x, y);
                    break 'rings;
                }
            }
        }
    }
    let mut tiles = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if region != 0 && map.passable(x, y) && regions.id_at(x, y) == region {
                tiles.push((x as u16, y as u16));
            }
        }
    }
    tiles
}

fn pick(rng: &mut Pcg32, tiles: &[(u16, u16)]) -> (u16, u16) {
    let i = rng.below(tiles.len() as u32) as usize;
    tiles.get(i).copied().unwrap_or((0, 0))
}

/// Days since 1970-01-01 to a civil date (proleptic Gregorian).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[allow(clippy::disallowed_methods)] // tools may read the clock
fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (y, m, d) = civil((secs / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

fn commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map_or_else(
            || "unknown".to_owned(),
            |o| String::from_utf8_lossy(&o.stdout).trim().to_owned(),
        )
}

/// Run the subcommand.
#[allow(clippy::disallowed_methods)] // tools may read the clock
pub fn run(args: &[String]) -> Result<String, String> {
    let o = parse(args)?;
    let data = generate_local(&EmbarkSummary::temperate_forest());
    let map = LocalMap::from_data(&data);
    let regions = Regions::build(&map);
    let tiles = region_tiles(&map, &regions);
    if tiles.is_empty() {
        return Err("map has no passable tiles".to_owned());
    }
    let mut rng = Pcg32::from_seed(7);
    let spawns: Vec<(u16, u16)> = (0..COLONISTS).map(|_| pick(&mut rng, &tiles)).collect();
    let mut sim = Sim::from_map(map, 1, 0, &spawns);
    let ids: Vec<_> = sim.colonists().iter().map(|(id, _)| id).collect();

    let mut total = std::time::Duration::ZERO;
    let mut worst = std::time::Duration::ZERO;
    let (mut searches, mut expanded) = (0u64, 0u64);
    let (mut max_searches, mut max_expanded, mut max_queue) = (0u32, 0u32, 0usize);
    let mut commands = Vec::new();
    for _ in 0..o.ticks {
        commands.clear();
        for &id in &ids {
            if sim.colonist(id).is_some_and(|c| c.target.is_none()) {
                let (x, y) = pick(&mut rng, &tiles);
                commands.push(Command::MoveColonist { id, x, y });
            }
        }
        let start = Instant::now();
        sim.step(&commands);
        let dt = start.elapsed();
        sim.drain_events();
        total += dt;
        worst = worst.max(dt);
        let s = sim.path_stats();
        searches += u64::from(s.searches);
        expanded += u64::from(s.expanded);
        max_searches = max_searches.max(s.searches);
        max_expanded = max_expanded.max(s.expanded);
        max_queue = max_queue.max(sim.path_queue_len());
    }
    let ticks = f64::from(o.ticks);
    let mean_ms = total.as_secs_f64() * 1000.0 / ticks;
    let max_ms = worst.as_secs_f64() * 1000.0;
    let mut out = format!(
        "ticks {}, colonists {}, region tiles {}\n\
         mean tick {mean_ms:.3} ms, max tick {max_ms:.3} ms (limit {LIMIT_MS} ms mean)\n\
         path searches: total {searches}, mean {:.2}/tick, max {max_searches}/tick\n\
         nodes expanded: total {expanded}, mean {:.0}/tick, max {max_expanded}/tick\n\
         max queue {max_queue}, state hash {:016x}",
        o.ticks,
        ids.len(),
        tiles.len(),
        searches as f64 / ticks,
        expanded as f64 / ticks,
        sim.state_hash(),
    );
    if o.csv {
        let row = format!(
            "{},{},S2 300-colonist random walk mean tick,{mean_ms:.3},{LIMIT_MS},{}\n",
            today(),
            commit(),
            o.machine.replace(',', ";"),
        );
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(CSV_PATH)
            .map_err(|e| format!("cannot open {CSV_PATH}: {e}"))?;
        file.write_all(row.as_bytes())
            .map_err(|e| format!("cannot write {CSV_PATH}: {e}"))?;
        out.push_str(&format!("\nappended to {CSV_PATH}"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_date_known_answers() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_380), (2025, 10, 19));
    }

    #[test]
    fn bad_arguments_return_errors() {
        assert!(parse(&["--ticks".into()]).is_err());
        assert!(parse(&["--ticks".into(), "0".into()]).is_err());
        assert!(parse(&["--wat".into()]).is_err());
    }
}
