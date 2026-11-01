//! Throwing the dice and sticks of the race Families. The randomness lives here, not in the rules:
//! the rules crates and `bragd-games` only ever see the throw as an entry. The app throws for Play,
//! the Helper for engine matches.

use bragd_games::Family;

/// One byte from the operating system's random source.
fn os_byte() -> u8 {
    let mut b = [0u8; 1];
    // The OS source does not fail on any system Bragd runs on; a failure is a broken machine.
    getrandom::fill(&mut b).expect("the operating system's random source");
    b[0]
}

/// A number from the operating system's random source: the seed an engine match chooses its
/// openings by.
pub fn seed() -> u64 {
    u64::from_le_bytes(std::array::from_fn(|_| os_byte()))
}

/// A number in `0..n`, every one equally likely: bytes from the top, uneven slice are thrown
/// away (rejection sampling), so no face is favoured as `byte % n` would favour the low ones.
fn uniform(n: u8, next: &mut impl FnMut() -> u8) -> u8 {
    let limit = 256 - 256 % n as u16;
    loop {
        let b = next();
        if (b as u16) < limit {
            return b % n;
        }
    }
}

/// A number in `0..n` from two bytes, rejection-sampled like [`uniform`].
fn uniform16(n: u16, next: &mut impl FnMut() -> u8) -> u16 {
    let limit = 65536 - 65536 % n as u32;
    loop {
        let v = (next() as u32) << 8 | next() as u32;
        if v < limit {
            return (v % n as u32) as u16;
        }
    }
}

/// The Chess960 array a new Game starts from, 0–959, every one equally likely.
pub fn chess960_array() -> u16 {
    uniform16(960, &mut os_byte)
}

/// One throw of the Family's implements, written as its entry.
fn cast(family: Family, next: &mut impl FnMut() -> u8) -> Option<String> {
    match family {
        // Two dice, the higher first: `6-5`.
        Family::Backgammon => {
            let (a, b) = (uniform(6, next) + 1, uniform(6, next) + 1);
            Some(format!("{}-{}", a.max(b), a.min(b)))
        }
        // Four tetrahedral dice, each with two marked tips of four: the marked ones count.
        Family::Ur => Some((next() & 0xF).count_ones().to_string()),
        // Four sticks: the light faces up count, and none up counts five.
        Family::Senet => {
            let sides = (next() & 0xF).count_ones();
            Some(if sides == 0 { 5 } else { sides }.to_string())
        }
        _ => None,
    }
}

/// A throw the rules accept now: one of `allowed`, thrown again until it is (the opening roll
/// of backgammon takes no doubles).
pub fn throw(family: Family, allowed: &[String]) -> Option<String> {
    if allowed.is_empty() {
        return None;
    }
    let mut next = os_byte;
    (0..1000)
        .filter_map(|_| cast(family, &mut next))
        .find(|t| allowed.contains(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A byte source that hands out the given bytes in order.
    fn bytes(list: &[u8]) -> impl FnMut() -> u8 + '_ {
        let mut i = 0;
        move || {
            i += 1;
            list[i - 1]
        }
    }

    #[test]
    fn uniform_throws_away_the_uneven_top_and_favours_no_face() {
        // 252..=255 would make 0..=3 more likely than 4 and 5; they are thrown away.
        assert_eq!(uniform(6, &mut bytes(&[255, 252, 251])), 251 % 6);
        assert_eq!(uniform(6, &mut bytes(&[253, 0])), 0);
        // Every byte value once: each face gets the same number of accepted bytes.
        let mut counts = [0u32; 6];
        for b in 0..=255u8 {
            let one = [b, 0];
            let mut src = bytes(&one);
            let face = uniform(6, &mut src);
            if b < 252 {
                counts[face as usize] += 1;
            }
        }
        assert_eq!(counts, [42; 6]);
        // A power of two keeps every byte.
        assert_eq!(uniform(4, &mut bytes(&[255])), 3);
    }

    #[test]
    fn a_chess960_array_is_any_of_the_960_and_only_those() {
        // 65280..=65535 would favour the low arrays; they are thrown away.
        assert_eq!(uniform16(960, &mut bytes(&[0xFF, 0x00, 0x03, 0xC1])), 1);
        assert_eq!(uniform16(960, &mut bytes(&[0xFE, 0xFF])), 65279 % 960);
        let a = chess960_array();
        assert!(a < 960);
        assert!(bragd_games::replay::<&str>("chess960", Some(&bragd_games::chess960_start(a)), &[]).is_ok());
    }

    #[test]
    fn throws_cover_every_face_and_only_those() {
        let all = |family| {
            // Bytes under 252, so no die needs a third.
            let mut seen: Vec<String> = (0..252u8)
                .flat_map(|a| (0..252u8).map(move |b| [a, b]))
                .filter_map(|pair| cast(family, &mut bytes(&pair[..])))
                .collect::<Vec<_>>();
            seen.sort();
            seen.dedup();
            seen
        };
        assert_eq!(all(Family::Backgammon).len(), 21);
        assert_eq!(cast(Family::Backgammon, &mut bytes(&[5, 0])).as_deref(), Some("6-1"));
        assert_eq!(cast(Family::Ur, &mut bytes(&[0xF0])).as_deref(), Some("0"));
        assert_eq!(cast(Family::Senet, &mut bytes(&[0xF0])).as_deref(), Some("5"));
        assert_eq!(cast(Family::Chess, &mut bytes(&[1])), None);
    }

    #[test]
    fn throw_keeps_to_what_the_rules_allow() {
        let state = bragd_games::replay::<&str>("backgammon", None, &[]).unwrap();
        for _ in 0..50 {
            let t = throw(Family::Backgammon, &state.throws).unwrap();
            assert!(state.throws.contains(&t), "{t}");
        }
        assert_eq!(throw(Family::Ur, &[]), None);
        assert_eq!(throw(Family::Ur, &["3".to_string()]).as_deref(), Some("3"));
    }
}
