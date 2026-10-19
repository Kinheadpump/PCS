//! Game time (TECHNICAL_DESIGN §6.1) and the real-time accumulator.
//!
//! All values are integers. The accumulator is a pure function, so tests can run it
//! without a window. Pause and speed are game state, not `Sim` state.

/// Ticks in one game hour.
pub const TICKS_PER_HOUR: u64 = 100;
/// Game hours in one game day.
pub const HOURS_PER_DAY: u64 = 24;
/// Ticks in one game day.
pub const TICKS_PER_DAY: u64 = TICKS_PER_HOUR * HOURS_PER_DAY;
/// Game days in one season.
pub const DAYS_PER_SEASON: u64 = 10;
/// Game days in one year.
pub const DAYS_PER_YEAR: u64 = DAYS_PER_SEASON * 4;
/// Ticks in one year.
pub const TICKS_PER_YEAR: u64 = TICKS_PER_DAY * DAYS_PER_YEAR;
/// Maximum ticks that one frame may run. If the computer is too slow, the game slows down.
pub const MAX_TICKS_PER_FRAME: u32 = 60;

/// One season of the year.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    /// Spring (days 1 to 10).
    Spring,
    /// Summer (days 11 to 20).
    Summer,
    /// Autumn (days 21 to 30).
    Autumn,
    /// Winter (days 31 to 40).
    Winter,
}

/// A tick number split into calendar units. The game starts at tick 0:
/// year 1, day 1 of spring, hour 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameTime {
    /// The tick number.
    pub tick: u64,
    /// Year, from 1.
    pub year: u64,
    /// Season of the year.
    pub season: Season,
    /// Day in the season, from 1 to 10.
    pub day_of_season: u32,
    /// Day in the year, from 1 to 40.
    pub day_of_year: u32,
    /// Hour of the day, from 0 to 23.
    pub hour: u32,
    /// Tick in the hour, from 0 to 99.
    pub tick_of_hour: u32,
}

impl GameTime {
    /// Split a tick number into calendar units.
    pub fn from_tick(tick: u64) -> Self {
        let year0 = tick / TICKS_PER_YEAR;
        let in_year = tick % TICKS_PER_YEAR;
        let day0 = in_year / TICKS_PER_DAY;
        let in_day = in_year % TICKS_PER_DAY;
        let season = match day0 / DAYS_PER_SEASON {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            _ => Season::Winter,
        };
        Self {
            tick,
            year: year0 + 1,
            season,
            day_of_season: (day0 % DAYS_PER_SEASON) as u32 + 1,
            day_of_year: day0 as u32 + 1,
            hour: (in_day / TICKS_PER_HOUR) as u32,
            tick_of_hour: (in_day % TICKS_PER_HOUR) as u32,
        }
    }
}

/// Game speed. The player sets it. It is not part of the `Sim` state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speed {
    /// No ticks.
    Paused,
    /// 40 ticks per real second.
    X1,
    /// 120 ticks per real second.
    X3,
    /// 240 ticks per real second.
    X6,
}

impl Speed {
    /// Ticks per real second at this speed.
    pub fn ticks_per_second(self) -> u64 {
        match self {
            Speed::Paused => 0,
            Speed::X1 => 40,
            Speed::X3 => 120,
            Speed::X6 => 240,
        }
    }
}

/// Microseconds in one second.
const MICROS_PER_SECOND: u64 = 1_000_000;

/// Add the real time of one frame to the accumulator and return the ticks to run now.
///
/// `accum` holds the rest as (ticks x microseconds). It is always less than one second
/// of that unit after the call. At most [`MAX_TICKS_PER_FRAME`] ticks run; the backlog
/// above that is dropped, so the game slows down and does not freeze.
pub fn ticks_for_frame(accum: &mut u64, speed: Speed, frame_micros: u64) -> u32 {
    let tps = speed.ticks_per_second();
    if tps == 0 {
        *accum = 0;
        return 0;
    }
    *accum = accum.saturating_add(frame_micros.saturating_mul(tps));
    let ticks = *accum / MICROS_PER_SECOND;
    *accum %= MICROS_PER_SECOND;
    ticks.min(u64::from(MAX_TICKS_PER_FRAME)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_constants_match_the_design() {
        assert_eq!(TICKS_PER_DAY, 2_400);
        assert_eq!(TICKS_PER_YEAR, 96_000);
    }

    #[test]
    fn game_time_starts_on_day_one_of_spring() {
        let t = GameTime::from_tick(0);
        assert_eq!(
            (t.year, t.season, t.day_of_season, t.day_of_year, t.hour),
            (1, Season::Spring, 1, 1, 0)
        );
    }

    #[test]
    fn game_time_rolls_over_hours_days_seasons_years() {
        assert_eq!(GameTime::from_tick(99).hour, 0);
        assert_eq!(GameTime::from_tick(100).hour, 1);
        assert_eq!(GameTime::from_tick(2_399).hour, 23);
        let d = GameTime::from_tick(2_400);
        assert_eq!((d.day_of_year, d.hour), (2, 0));
        let s = GameTime::from_tick(10 * TICKS_PER_DAY);
        assert_eq!((s.season, s.day_of_season), (Season::Summer, 1));
        let w = GameTime::from_tick(39 * TICKS_PER_DAY + 2_399);
        assert_eq!(
            (w.season, w.day_of_season, w.day_of_year, w.hour),
            (Season::Winter, 10, 40, 23)
        );
        let y = GameTime::from_tick(TICKS_PER_YEAR);
        assert_eq!((y.year, y.season, y.day_of_year), (2, Season::Spring, 1));
    }

    fn run_seconds(speed: Speed, seconds: u64, frame_micros: u64) -> u64 {
        let mut accum = 0;
        let mut total = 0u64;
        let frames = seconds * MICROS_PER_SECOND / frame_micros;
        for _ in 0..frames {
            total += u64::from(ticks_for_frame(&mut accum, speed, frame_micros));
        }
        total
    }

    #[test]
    fn at_6x_60_real_seconds_run_14400_ticks_within_one_percent() {
        for frame_micros in [16_667, 8_333, 6_944, 33_333] {
            let total = run_seconds(Speed::X6, 60, frame_micros);
            assert!(
                (14_256..=14_544).contains(&total),
                "frame {frame_micros} us gave {total} ticks"
            );
        }
    }

    #[test]
    fn speeds_give_the_design_tick_rates() {
        assert_eq!(run_seconds(Speed::X1, 10, 10_000), 400);
        assert_eq!(run_seconds(Speed::X3, 10, 10_000), 1_200);
        assert_eq!(run_seconds(Speed::Paused, 10, 10_000), 0);
    }

    #[test]
    fn one_slow_frame_runs_at_most_60_ticks_and_drops_the_backlog() {
        let mut accum = 0;
        assert_eq!(
            ticks_for_frame(&mut accum, Speed::X6, 5 * MICROS_PER_SECOND),
            MAX_TICKS_PER_FRAME
        );
        assert!(accum < MICROS_PER_SECOND);
        assert_eq!(ticks_for_frame(&mut accum, Speed::X6, 0), 0);
    }
}
