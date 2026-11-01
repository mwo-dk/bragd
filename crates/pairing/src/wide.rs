//! Fixed-width two's-complement integers of 64-bit limbs; [`Wide`] has 512 bits.
//!
//! Pairing criteria are compared lexicographically, so each criterion gets its
//! own field of bits in one matching weight. With many score levels those
//! fields need more than 128 bits; 512 bits holds every realistic tournament.

use std::cmp::Ordering;
use std::ops::{Add, Neg, Sub};

/// The number of bits a [`Wide`] can hold without overflow, sign bit excluded.
pub const BITS: u32 = 64 * 8 - 1;

/// The weight of every bracket's criteria.
pub type Wide = Limbs<8>;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Limbs<const L: usize>([u64; L]);

impl<const L: usize> Limbs<L> {
    pub const ZERO: Self = Limbs([0; L]);

    pub fn from_u64(v: u64) -> Self {
        let mut w = Self::ZERO;
        w.0[0] = v;
        w
    }

    /// `v << shift`, for building a criterion field. `shift + 64` must fit in [`BITS`].
    pub fn shifted(v: u64, shift: u32) -> Self {
        let mut w = Self::ZERO;
        let (limb, bit) = ((shift / 64) as usize, shift % 64);
        w.0[limb] = v << bit;
        if bit != 0 && limb + 1 < L {
            w.0[limb + 1] = v >> (64 - bit);
        }
        w
    }

    /// The `width` bits starting at bit `shift`. Only for non-negative values.
    pub fn field(self, shift: u32, width: u32) -> u64 {
        let (limb, bit) = ((shift / 64) as usize, shift % 64);
        let mut v = self.0[limb] >> bit;
        if bit != 0 && limb + 1 < L {
            v |= self.0[limb + 1] << (64 - bit);
        }
        if width >= 64 {
            v
        } else {
            v & ((1u64 << width) - 1)
        }
    }

    /// The same non-negative value in `M` limbs, when it is below `2^bits`.
    pub fn resize<const M: usize>(self, bits: u32) -> Option<Limbs<M>> {
        let mut r = Limbs([0; M]);
        for (i, &l) in self.0.iter().enumerate() {
            let top = (i as u32 + 1) * 64;
            let room = bits.saturating_sub(i as u32 * 64);
            let fits = if top <= bits {
                true
            } else {
                room < 64 && l >> room == 0
            };
            if !fits || (l != 0 && i >= M) {
                return None;
            }
            if i < M {
                r.0[i] = l;
            }
        }
        Some(r)
    }

    pub fn is_negative(self) -> bool {
        self.0[L - 1] >> 63 == 1
    }

    /// Halves an even value (arithmetic shift right by one).
    pub fn half(self) -> Self {
        let mut r = Self::ZERO;
        let sign = self.0[L - 1] & (1 << 63);
        for i in 0..L {
            let hi = if i + 1 < L { self.0[i + 1] << 63 } else { sign };
            r.0[i] = (self.0[i] >> 1) | hi;
        }
        r
    }
}

impl<const L: usize> Add for Limbs<L> {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        let mut r = Self::ZERO;
        let mut carry = false;
        for i in 0..L {
            let (s, c1) = self.0[i].overflowing_add(o.0[i]);
            let (s, c2) = s.overflowing_add(carry as u64);
            r.0[i] = s;
            carry = c1 || c2;
        }
        r
    }
}

impl<const L: usize> Neg for Limbs<L> {
    type Output = Self;
    fn neg(self) -> Self {
        let mut r = Self::ZERO;
        for i in 0..L {
            r.0[i] = !self.0[i];
        }
        r + Self::from_u64(1)
    }
}

impl<const L: usize> Sub for Limbs<L> {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        self + (-o)
    }
}

impl<const L: usize> Ord for Limbs<L> {
    fn cmp(&self, o: &Self) -> Ordering {
        match (self.is_negative(), o.is_negative()) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        }
        for i in (0..L).rev() {
            match self.0[i].cmp(&o.0[i]) {
                Ordering::Equal => continue,
                ord => return ord,
            }
        }
        Ordering::Equal
    }
}

impl<const L: usize> PartialOrd for Limbs<L> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl<const L: usize> std::fmt::Debug for Limbs<L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_negative() {
            write!(f, "-")?;
            return std::fmt::Debug::fmt(&-*self, f);
        }
        write!(f, "0x")?;
        let top = (0..L).rev().find(|&i| self.0[i] != 0).unwrap_or(0);
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
    fn resizes_when_the_value_fits() {
        let v = Wide::shifted(0xabc, 100) + Wide::from_u64(7);
        let small: Limbs<2> = v.resize(118).unwrap();
        assert_eq!(small.resize::<8>(128), Some(v));
        assert_eq!(v.resize::<2>(100), None);
        assert_eq!(Wide::shifted(1, 200).resize::<2>(118), None);
        assert_eq!(w(-3).resize::<2>(118), None);
        assert_eq!(
            Wide::shifted(1, 200).resize::<4>(250),
            Some(Limbs::<4>::shifted(1, 200))
        );
    }

    #[test]
    fn fields_read_back() {
        let v = Wide::shifted(5, 3) + Wide::shifted(0b1011, 62) + Wide::shifted(9, 500);
        assert_eq!(v.field(3, 3), 5);
        assert_eq!(v.field(62, 4), 0b1011);
        assert_eq!(v.field(500, 4), 9);
    }
}
