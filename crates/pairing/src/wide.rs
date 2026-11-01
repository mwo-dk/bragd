//! A fixed 512-bit two's-complement integer.
//!
//! Pairing criteria are compared lexicographically, so each criterion gets its
//! own field of bits in one matching weight. With many score levels those
//! fields need more than 128 bits; 512 bits holds every realistic tournament.

use std::cmp::Ordering;
use std::ops::{Add, Neg, Sub};

const LIMBS: usize = 8;

/// The number of bits a [`Wide`] can hold without overflow, sign bit excluded.
pub const BITS: u32 = 64 * LIMBS as u32 - 1;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Wide([u64; LIMBS]);

impl Wide {
    pub const ZERO: Wide = Wide([0; LIMBS]);

    pub fn from_u64(v: u64) -> Wide {
        let mut w = Wide::ZERO;
        w.0[0] = v;
        w
    }

    /// `v << shift`, for building a criterion field. `shift + 64` must fit in [`BITS`].
    pub fn shifted(v: u64, shift: u32) -> Wide {
        let mut w = Wide::ZERO;
        let (limb, bit) = ((shift / 64) as usize, shift % 64);
        w.0[limb] = v << bit;
        if bit != 0 && limb + 1 < LIMBS {
            w.0[limb + 1] = v >> (64 - bit);
        }
        w
    }

    /// The `width` bits starting at bit `shift`. Only for non-negative values.
    pub fn field(self, shift: u32, width: u32) -> u64 {
        let (limb, bit) = ((shift / 64) as usize, shift % 64);
        let mut v = self.0[limb] >> bit;
        if bit != 0 && limb + 1 < LIMBS {
            v |= self.0[limb + 1] << (64 - bit);
        }
        if width >= 64 {
            v
        } else {
            v & ((1u64 << width) - 1)
        }
    }

    pub fn is_negative(self) -> bool {
        self.0[LIMBS - 1] >> 63 == 1
    }

    /// Halves an even value (arithmetic shift right by one).
    pub fn half(self) -> Wide {
        let mut r = Wide::ZERO;
        let sign = self.0[LIMBS - 1] & (1 << 63);
        for i in 0..LIMBS {
            let hi = if i + 1 < LIMBS {
                self.0[i + 1] << 63
            } else {
                sign
            };
            r.0[i] = (self.0[i] >> 1) | hi;
        }
        r
    }
}

impl Add for Wide {
    type Output = Wide;
    fn add(self, o: Wide) -> Wide {
        let mut r = Wide::ZERO;
        let mut carry = false;
        for i in 0..LIMBS {
            let (s, c1) = self.0[i].overflowing_add(o.0[i]);
            let (s, c2) = s.overflowing_add(carry as u64);
            r.0[i] = s;
            carry = c1 || c2;
        }
        r
    }
}

impl Neg for Wide {
    type Output = Wide;
    fn neg(self) -> Wide {
        let mut r = Wide::ZERO;
        for i in 0..LIMBS {
            r.0[i] = !self.0[i];
        }
        r + Wide::from_u64(1)
    }
}

impl Sub for Wide {
    type Output = Wide;
    fn sub(self, o: Wide) -> Wide {
        self + (-o)
    }
}

impl Ord for Wide {
    fn cmp(&self, o: &Wide) -> Ordering {
        match (self.is_negative(), o.is_negative()) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        }
        for i in (0..LIMBS).rev() {
            match self.0[i].cmp(&o.0[i]) {
                Ordering::Equal => continue,
                ord => return ord,
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for Wide {
    fn partial_cmp(&self, o: &Wide) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl std::fmt::Debug for Wide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_negative() {
            write!(f, "-")?;
            return std::fmt::Debug::fmt(&-*self, f);
        }
        write!(f, "0x")?;
        let top = (0..LIMBS).rev().find(|&i| self.0[i] != 0).unwrap_or(0);
        write!(f, "{:x}", self.0[top])?;
        for i in (0..top).rev() {
            write!(f, "{:016x}", self.0[i])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(v: i64) -> Wide {
        if v < 0 {
            -Wide::from_u64(v.unsigned_abs())
        } else {
            Wide::from_u64(v as u64)
        }
    }

    #[test]
    fn small_values_behave_like_i64() {
        let vals = [-7i64, -2, -1, 0, 1, 2, 9, 1 << 40, -(1 << 40)];
        for &a in &vals {
            for &b in &vals {
                assert_eq!(w(a) + w(b), w(a + b), "{a}+{b}");
                assert_eq!(w(a) - w(b), w(a - b), "{a}-{b}");
                assert_eq!(w(a).cmp(&w(b)), a.cmp(&b), "{a} cmp {b}");
            }
            if a % 2 == 0 {
                assert_eq!(w(a).half(), w(a / 2), "{a}/2");
            }
        }
    }

    #[test]
    fn carries_cross_limbs() {
        let a = Wide::shifted(u64::MAX, 0) + Wide::from_u64(1);
        assert_eq!(a, Wide::shifted(1, 64));
        assert_eq!(a - Wide::from_u64(1), Wide::from_u64(u64::MAX));
        assert!(Wide::shifted(1, 400) > Wide::shifted(u64::MAX, 300));
        assert_eq!(Wide::shifted(6, 300).half(), Wide::shifted(3, 300));
    }

    #[test]
    fn fields_read_back() {
        let v = Wide::shifted(5, 3) + Wide::shifted(0b1011, 62) + Wide::shifted(9, 500);
        assert_eq!(v.field(3, 3), 5);
        assert_eq!(v.field(62, 4), 0b1011);
        assert_eq!(v.field(500, 4), 9);
    }
}
