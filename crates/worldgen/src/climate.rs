//! Steps 6 and 7: temperature and moisture.

use crate::noise::fbm;
use crate::world::{WORLD_SIZE, WorldParams, world_grid};
use base::Grid;
use base::rng::mix;

/// Mean sea-level temperature in degrees Celsius from the equator (first) to the pole (last).
const TEMP_BY_LATITUDE: [i32; 17] = [
    30, 30, 29, 28, 27, 25, 23, 20, 17, 13, 9, 5, 0, -6, -12, -19, -26,
];

/// Half of the yearly temperature range at the coast, from the equator to the pole.
const AMP_BY_LATITUDE: [i32; 9] = [3, 4, 6, 8, 10, 12, 14, 15, 16];

/// Base moisture (0 to 255) from the equator to the pole: wet tropics, dry subtropics,
/// wet mid latitudes, dry poles.
const MOISTURE_BY_LATITUDE: [i32; 17] = [
    80, 80, 70, 55, 35, 25, 25, 35, 55, 80, 90, 90, 80, 60, 40, 25, 15,
];

/// Limit of the moisture carried by the wind (1/16 of a moisture unit).
const MOISTURE_CAP: i32 = 4096;

/// Moisture that land adds back to the air each cell (plants and ground water).
const LAND_RECYCLE: i32 = 8;

/// Scale between fallen rain and the moisture value.
const RAIN_SCALE: i32 = 160;

/// Linear interpolation in a table that runs from the equator (row y = middle) to the poles.
pub fn latitude_value(table: &[i32], y: i32, size: i32) -> i32 {
    let d = (2 * y - (size - 1)).abs();
    let span = (size - 1).max(1);
    let last = table.len() as i32 - 1;
    let scaled = d * last;
    let i = (scaled / span).min(last);
    let frac = scaled % span;
    let a = table[i as usize];
    let b = table[(i + 1).min(last) as usize];
    a + (b - a) * frac / span
}

/// Direction of the prevailing wind in row y: +1 blows to the east, -1 to the west.
/// Trade winds (under 30 degrees) and polar winds (over 60 degrees) blow to the west.
/// Westerlies (30 to 60 degrees) blow to the east.
pub fn wind_dir(y: i32, size: i32) -> i32 {
    let d = (2 * y - (size - 1)).abs();
    let degrees = d * 90 / (size - 1).max(1);
    if (30..60).contains(&degrees) { 1 } else { -1 }
}

/// Distance to the nearest ocean cell in cells (chamfer 3-4, two passes). Ocean cells are 0.
fn ocean_distance(height: &[i16], sea_level: i16) -> Vec<u16> {
    let w = WORLD_SIZE as usize;
    let far = u16::MAX / 2;
    let mut dist: Vec<u16> = height
        .iter()
        .map(|&h| if h < sea_level { 0 } else { far })
        .collect();
    for y in 0..w {
        for x in 0..w {
            let i = y * w + x;
            let mut best = dist[i];
            if x > 0 {
                best = best.min(dist[i - 1] + 3);
            }
            if y > 0 {
                best = best.min(dist[i - w] + 3);
                if x > 0 {
                    best = best.min(dist[i - w - 1] + 4);
                }
                if x + 1 < w {
                    best = best.min(dist[i - w + 1] + 4);
                }
            }
            dist[i] = best;
        }
    }
    for y in (0..w).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            let mut best = dist[i];
            if x + 1 < w {
                best = best.min(dist[i + 1] + 3);
            }
            if y + 1 < w {
                best = best.min(dist[i + w] + 3);
                if x + 1 < w {
                    best = best.min(dist[i + w + 1] + 4);
                }
                if x > 0 {
                    best = best.min(dist[i + w - 1] + 4);
                }
            }
            dist[i] = best;
        }
    }
    dist.iter().map(|&d| d / 3).collect()
}

/// Make the mean temperature (degrees Celsius) and the yearly amplitude of each cell.
/// Temperature = latitude table - lapse rate x height above sea + noise.
pub fn make_temperature(
    seed: u64,
    height: &Grid<i16>,
    sea_level: i16,
    params: &WorldParams,
) -> (Grid<i8>, Grid<u8>) {
    let size = WORLD_SIZE as i32;
    let hs = height.as_slice();
    let dist = ocean_distance(hs, sea_level);
    let noise_seed = mix(seed ^ 1);
    let n = hs.len();
    let mut mean = Vec::with_capacity(n);
    let mut amp = Vec::with_capacity(n);
    for y in 0..size {
        let base = latitude_value(&TEMP_BY_LATITUDE, y, size);
        let amp_base = latitude_value(&AMP_BY_LATITUDE, y, size);
        for x in 0..size {
            let i = (y * size + x) as usize;
            let rel = (i32::from(hs[i]) - i32::from(sea_level)).max(0);
            let lapse = rel * params.lapse_per_10k / 10000;
            let noise = (i32::from(fbm(noise_seed, x, y, 64, 3)) - 32768) / 8192;
            mean.push((base - lapse + noise).clamp(-60, 60) as i8);
            let inland = i32::from(dist[i]).min(40) / 4;
            let a = if hs[i] < sea_level {
                amp_base / 2
            } else {
                amp_base + inland
            };
            amp.push(a.clamp(0, 255) as u8);
        }
    }
    (world_grid(mean, 0), world_grid(amp, 0))
}

/// Make the moisture (0 to 255) of each cell. In each row the wind carries moisture from
/// the ocean over the land. Land cells take rain from it. Rain is stronger where the land
/// climbs, so cells behind a mountain range get less (rain shadow).
pub fn make_moisture(
    seed: u64,
    height: &Grid<i16>,
    sea_level: i16,
    temperature: &Grid<i8>,
    params: &WorldParams,
) -> Grid<u8> {
    let size = WORLD_SIZE as i32;
    let hs = height.as_slice();
    let temps = temperature.as_slice();
    let noise_seed = mix(seed ^ 1);
    let mut out = vec![0u8; hs.len()];
    for y in 0..size {
        let dir = wind_dir(y, size);
        let base = latitude_value(&MOISTURE_BY_LATITUDE, y, size);
        let mut carried = 0i32;
        let mut prev_rel = 0i32;
        for step in 0..size {
            let x = if dir > 0 { step } else { size - 1 - step };
            let i = (y * size + x) as usize;
            if hs[i] < sea_level {
                let evap = ((i32::from(temps[i]) + 10) * 3).clamp(6, 120);
                carried = (carried + evap).min(MOISTURE_CAP);
                out[i] = 255;
                prev_rel = 0;
                continue;
            }
            let rel = i32::from(hs[i]) - i32::from(sea_level);
            let climb = (rel - prev_rel).max(0);
            let rate = (params.rain_base + params.rain_climb * climb).clamp(0, 700);
            let rain = carried * rate / 1000;
            carried = (carried - rain + LAND_RECYCLE).min(MOISTURE_CAP);
            let noise = (i32::from(fbm(noise_seed, x, y, 32, 3)) - 32768) / 2730;
            out[i] = (base + noise + rain * 255 / RAIN_SCALE).clamp(0, 255) as u8;
            prev_rel = rel;
        }
    }
    world_grid(out, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latitude_table_is_symmetric_and_ends_at_the_pole() {
        let size = WORLD_SIZE as i32;
        let v = |y| latitude_value(&TEMP_BY_LATITUDE, y, size);
        assert_eq!(v(0), -26);
        assert_eq!(v(size - 1), -26);
        assert_eq!(v(0), v(size - 1));
        assert!(v(size / 2) >= 29);
    }

    #[test]
    fn wind_bands_alternate() {
        let size = WORLD_SIZE as i32;
        let mid = size / 2;
        assert_eq!(wind_dir(mid, size), -1);
        assert_eq!(wind_dir(mid + 60, size), 1);
        assert_eq!(wind_dir(mid - 60, size), 1);
        assert_eq!(wind_dir(2, size), -1);
    }

    #[test]
    fn ocean_distance_counts_cells() {
        let w = WORLD_SIZE as usize;
        let mut cells = vec![100i16; w * w];
        cells[0] = -10;
        let d = ocean_distance(&cells, 0);
        assert_eq!(d[0], 0);
        assert_eq!(d[5], 5);
        assert_eq!(d[w + 1], 1);
    }
}
