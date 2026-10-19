//! 16.16 fixed-point number. Use it only where a fraction is needed.

use serde::{Deserialize, Serialize};
use std::ops::{Add, Neg, Sub};

/// Signed 16.16 fixed-point number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Fixed(i32);

impl Fixed {
    /// Number of fraction bits.
    pub const FRAC_BITS: u32 = 16;
    /// The value 0.
    pub const ZERO: Fixed = Fixed(0);
    /// The value 1.
    pub const ONE: Fixed = Fixed(1 << Self::FRAC_BITS);

    /// Make a value from its raw bits.
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    /// Return the raw bits.
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// Make a value from an integer. Saturates at the limits.
    pub const fn from_int(v: i32) -> Self {
        Self(v.saturating_mul(1 << Self::FRAC_BITS))
    }

    /// Make the value `num / den`, rounded toward zero. Return zero if `den` is 0.
    pub fn from_ratio(num: i32, den: i32) -> Self {
        if den == 0 {
            return Self::ZERO;
        }
        let v = (i64::from(num) << Self::FRAC_BITS) / i64::from(den);
        Self(clamp_i64(v))
    }

    /// Round toward minus infinity to an integer.
    pub const fn floor(self) -> i32 {
        self.0 >> Self::FRAC_BITS
    }

    /// Multiply. Saturates. The result is rounded toward minus infinity.
    pub fn mul_fixed(self, rhs: Self) -> Self {
        Self(clamp_i64(
            (i64::from(self.0) * i64::from(rhs.0)) >> Self::FRAC_BITS,
        ))
    }

    /// Divide. Saturates. Return zero if `rhs` is zero.
    pub fn div_fixed(self, rhs: Self) -> Self {
        if rhs.0 == 0 {
            return Self::ZERO;
        }
        Self(clamp_i64(
            (i64::from(self.0) << Self::FRAC_BITS) / i64::from(rhs.0),
        ))
    }

    /// Add. Saturates.
    pub fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    /// Subtract. Saturates.
    pub fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

fn clamp_i64(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

impl Add for Fixed {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        self.saturating_add(rhs)
    }
}

impl Sub for Fixed {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        self.saturating_sub(rhs)
    }
}

impl Neg for Fixed {
    type Output = Self;
    fn neg(self) -> Self {
        Self(self.0.saturating_neg())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_round_trip() {
        assert_eq!(Fixed::from_int(5).floor(), 5);
        assert_eq!(Fixed::from_int(-5).floor(), -5);
        assert_eq!(Fixed::ONE.raw(), 65536);
    }

    #[test]
    fn ratio_and_floor() {
        assert_eq!(Fixed::from_ratio(1, 2).raw(), 32768);
        assert_eq!(Fixed::from_ratio(-1, 2).floor(), -1);
        assert_eq!(Fixed::from_ratio(1, 0), Fixed::ZERO);
    }

    #[test]
    fn mul_and_div() {
        let a = Fixed::from_ratio(3, 2);
        assert_eq!(a.mul_fixed(Fixed::from_int(2)), Fixed::from_int(3));
        assert_eq!(Fixed::from_int(3).div_fixed(Fixed::from_int(2)), a);
        assert_eq!(a.div_fixed(Fixed::ZERO), Fixed::ZERO);
    }

    #[test]
    fn arithmetic_saturates() {
        let big = Fixed::from_raw(i32::MAX);
        assert_eq!(big + Fixed::ONE, big);
        assert_eq!(big.mul_fixed(Fixed::from_int(2)), big);
        assert_eq!(
            Fixed::from_raw(i32::MIN) - Fixed::ONE,
            Fixed::from_raw(i32::MIN)
        );
        assert_eq!(Fixed::from_int(i32::MAX), big);
    }

    #[test]
    fn add_sub_neg() {
        let a = Fixed::from_int(2);
        let b = Fixed::from_int(3);
        assert_eq!(a + b, Fixed::from_int(5));
        assert_eq!(a - b, -Fixed::ONE);
    }
}
