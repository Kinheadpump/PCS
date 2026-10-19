//! Own random number generators. Known-answer tests freeze the output.

use serde::{Deserialize, Serialize};

const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
const PCG_MULT: u64 = 6_364_136_223_846_793_005;

/// Stateless 64-bit mixing function (the SplitMix64 finalizer).
pub fn mix(value: u64) -> u64 {
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// SplitMix64 generator. Use it for seeds and for hashing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Make a generator from a seed.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Return the next 64-bit value.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_GAMMA);
        mix(self.state)
    }
}

/// PCG32 (XSH-RR) generator. Use it for random streams.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    /// Make a generator from a state seed and a stream number (reference PCG seeding).
    pub fn new(init_state: u64, stream: u64) -> Self {
        let mut rng = Self {
            state: 0,
            inc: (stream << 1) | 1,
        };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(init_state);
        rng.next_u32();
        rng
    }

    /// Make a generator from one seed. SplitMix64 derives the state and the stream.
    pub fn from_seed(seed: u64) -> Self {
        let mut sm = SplitMix64::new(seed);
        let state = sm.next_u64();
        let stream = sm.next_u64();
        Self::new(state, stream)
    }

    /// Return the next 32-bit value.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(PCG_MULT).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Return the next 64-bit value.
    pub fn next_u64(&mut self) -> u64 {
        (u64::from(self.next_u32()) << 32) | u64::from(self.next_u32())
    }

    /// Return a value in `0..bound`, without modulo bias. Return 0 if `bound` is 0.
    pub fn below(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let r = self.next_u32();
            if r >= threshold {
                return r % bound;
            }
        }
    }

    /// Return a value in `lo..hi`. Return `lo` if the range is empty.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        let span = hi.wrapping_sub(lo) as u32;
        lo.wrapping_add(self.below(span) as i32)
    }

    /// Return true with probability `num / den`.
    pub fn chance(&mut self, num: u32, den: u32) -> bool {
        self.below(den) < num
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_known_answers_for_three_seeds() {
        let cases: [(u64, [u64; 3]); 3] = [
            (
                0,
                [
                    0xe220_a839_7b1d_cdaf,
                    0x6e78_9e6a_a1b9_65f4,
                    0x06c4_5d18_8009_454f,
                ],
            ),
            (
                1,
                [
                    0x910a_2dec_8902_5cc1,
                    0xbeeb_8da1_658e_ec67,
                    0xf893_a2ee_fb32_555e,
                ],
            ),
            (
                0xDEAD_BEEF,
                [
                    0x4adf_b90f_68c9_eb9b,
                    0xde58_6a31_41a1_0922,
                    0x021f_bc2f_8e1c_fc1d,
                ],
            ),
        ];
        for (seed, expected) in cases {
            let mut rng = SplitMix64::new(seed);
            for want in expected {
                assert_eq!(rng.next_u64(), want, "seed {seed}");
            }
        }
    }

    #[test]
    fn pcg32_reference_known_answers() {
        let mut rng = Pcg32::new(42, 54);
        let expected = [
            0xa15c_02b7,
            0x7b47_f409,
            0xba1d_3330,
            0x83d2_f293,
            0xbfa4_784b,
            0xcbed_606e,
        ];
        for want in expected {
            assert_eq!(rng.next_u32(), want);
        }
    }

    #[test]
    fn pcg32_from_seed_is_repeatable_and_differs_by_seed() {
        let take = |seed| {
            let mut r = Pcg32::from_seed(seed);
            (0..4).map(|_| r.next_u32()).collect::<Vec<_>>()
        };
        assert_eq!(take(1), take(1));
        assert_ne!(take(1), take(2));
    }

    #[test]
    fn below_and_range_stay_in_bounds() {
        let mut rng = Pcg32::from_seed(7);
        for _ in 0..1000 {
            assert!(rng.below(10) < 10);
            let v = rng.range(-5, 5);
            assert!((-5..5).contains(&v));
        }
        assert_eq!(rng.below(0), 0);
        assert_eq!(rng.range(3, 3), 3);
    }

    #[test]
    fn mix_is_the_splitmix_finalizer() {
        assert_eq!(mix(GOLDEN_GAMMA), 0xe220_a839_7b1d_cdaf);
    }
}
