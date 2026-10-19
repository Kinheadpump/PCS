//! Pause and speed. They are game state, not `Sim` state (TECHNICAL_DESIGN §6.1).

use sim::Speed;
use sim::time::ticks_for_frame;

/// Speeds in button order.
pub const SPEEDS: [Speed; 4] = [Speed::Paused, Speed::X1, Speed::X3, Speed::X6];

/// Button label of a speed.
pub fn speed_label(speed: Speed) -> &'static str {
    match speed {
        Speed::Paused => "Pause",
        Speed::X1 => "1x",
        Speed::X3 => "3x",
        Speed::X6 => "6x",
    }
}

/// The real-time clock of the game loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    speed: Speed,
    /// Speed that Space returns to after a pause.
    resume: Speed,
    accum: u64,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            speed: Speed::X1,
            resume: Speed::X1,
            accum: 0,
        }
    }
}

impl Clock {
    /// The current speed.
    pub fn speed(&self) -> Speed {
        self.speed
    }

    /// Set the speed. A running speed is remembered for the pause toggle.
    pub fn set_speed(&mut self, speed: Speed) {
        if speed != Speed::Paused {
            self.resume = speed;
        }
        self.speed = speed;
    }

    /// Pause, or return to the speed before the pause.
    pub fn toggle_pause(&mut self) {
        self.speed = if self.speed == Speed::Paused {
            self.resume
        } else {
            Speed::Paused
        };
    }

    /// Ticks to run for a frame of `frame_micros` microseconds.
    pub fn advance(&mut self, frame_micros: u64) -> u32 {
        ticks_for_frame(&mut self.accum, self.speed, frame_micros)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_toggles_between_pause_and_the_last_running_speed() {
        let mut c = Clock::default();
        c.set_speed(Speed::X3);
        c.toggle_pause();
        assert_eq!(c.speed(), Speed::Paused);
        c.toggle_pause();
        assert_eq!(c.speed(), Speed::X3);
    }

    #[test]
    fn pausing_with_the_button_keeps_the_resume_speed() {
        let mut c = Clock::default();
        c.set_speed(Speed::X6);
        c.set_speed(Speed::Paused);
        c.toggle_pause();
        assert_eq!(c.speed(), Speed::X6);
    }

    #[test]
    fn paused_clock_runs_no_ticks_and_6x_runs_240_per_second() {
        let mut c = Clock::default();
        c.set_speed(Speed::Paused);
        assert_eq!(c.advance(1_000_000), 0);
        c.set_speed(Speed::X6);
        let total: u32 = (0..100).map(|_| c.advance(10_000)).sum();
        assert_eq!(total, 240);
    }

    #[test]
    fn every_speed_has_a_distinct_label() {
        for (i, a) in SPEEDS.iter().enumerate() {
            for b in &SPEEDS[i + 1..] {
                assert_ne!(speed_label(*a), speed_label(*b));
            }
        }
    }
}
