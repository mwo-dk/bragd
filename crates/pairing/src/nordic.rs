//! The Nordic Swiss system: the Danish Chess Union's *Dansk Schweizer*,
//! Skakhåndbogen 4.9.3 and the arbiter guide 4.9.4 (version 15.5.2, 2015; the
//! same text in 15.10, 2019, the last handbook). Clubs call it "Nordisk".
//!
//! - Start numbers go by strength, number 1 the strongest (3.1); there is no draw.
//! - Players are split into score pools, paired from the top (4.1-4.2). A pool
//!   is halved into a strong half `a` and a weak half `b`, and `a1` meets `b1`,
//!   `a2` meets `b2`, and so on (5.1, and 5.2 of the 2007 text).
//! - In a pool with an odd number the middle player floats (6.1). Before the
//!   next pool is paired, each floater meets the nearest lower player not yet
//!   met (5.4).
//! - When a pair has met before, the players of `b` move up, undoing the pair
//!   before when needed; then players are moved between `a` and `b`; then other
//!   floaters are chosen; and at last the whole pool floats (5.3, 6.2-6.3).
//!   The first pairing found is taken. Rule 5.2 of the 2011-2019 text asks
//!   for the one meeting most colour preferences, but the pairings DSU's own
//!   system made for these events take the first (`tests/nordic.rs`).
//! - A pool, or a floater's pair, is undone only when the players after it
//!   cannot be paired at all (4.9.4, example 11; 9.1-9.3).
//! - Meeting twice is never allowed (1.2), nor is a pair that leaves a strong
//!   colour preference unmet, except between two players over 50 % in the
//!   last round.
//! - With an odd number a player "X" is added as the weakest; whoever meets X
//!   has the bye, scores a win and counts as having had White (2.1-2.2).
//!
//! Colours: in round 1 the weakest player of `b` has White and the colours
//! alternate up through `b` (7.1). After that the player with fewer Whites has
//! White; with equal counts, the one with fewer Whites over the last round,
//! then the last two, and so on; and failing that the player with more
//! points, or the stronger on equal points, has the colour opposite to his
//! last (7.2-7.4).

use crate::matching::max_weight_matching;
use crate::{Board, Bracket, Colour, Criterion, Error, Pairing, Player, Round, Rule, Settings};
use std::collections::HashMap;

/// The added weakest player.
const X: u32 = u32::MAX;

struct P {
    tpn: u32,
    score: u32,
    opponents: Vec<u32>,
    /// Colours of played games in order; the bye counts as White.
    colours: Vec<Colour>,
    /// Preferred colour, and whether the preference is strong.
    pref: Option<(Colour, bool)>,
    top: bool,
}

/// Pairs one round of a Nordic Swiss (DSU's Dansk Schweizer).
pub fn pair(settings: &Settings, players: &[Player], present: &[u32]) -> Result<Pairing, Error> {
    let mut by_tpn = HashMap::new();
    let done = players.first().map_or(0, |p| p.history.len());
    for p in players {
        if p.history.len() != done {
            return Err(Error::UnevenHistory(p.tpn));
        }
        if by_tpn.insert(p.tpn, p).is_some() {
            return Err(Error::DuplicatePlayer(p.tpn));
        }
    }
    if done as u32 >= settings.rounds {
        return Err(Error::TournamentOver);
    }
    let mut ps: Vec<P> = Vec::with_capacity(present.len() + 1);
    for &t in present {
        let p = by_tpn.get(&t).ok_or(Error::UnknownPlayer(t))?;
        ps.push(describe(settings, p, done));
    }
    ps.sort_by_key(|p| p.tpn);
    if let Some(w) = ps.windows(2).find(|w| w[0].tpn == w[1].tpn) {
        return Err(Error::DuplicatePlayer(w[0].tpn));
    }
    if ps.len() % 2 == 1 {
        let had_bye: Vec<u32> = present
            .iter()
            .copied()
            .filter(|t| {
                by_tpn[t]
                    .history
                    .iter()
                    .any(|r| matches!(r, Round::PairingBye { .. }))
            })
            .collect();
        // X sits in the lowest pool as its weakest player.
        let lowest = ps.iter().map(|p| p.score).min().unwrap_or(0);
        ps.push(P {
            tpn: X,
            score: lowest,
            opponents: had_bye,
            colours: Vec::new(),
            pref: None,
            top: false,
        });
    }
    // Pools from the top; within a pool by start number, X last.
    ps.sort_by(|a, b| b.score.cmp(&a.score).then(a.tpn.cmp(&b.tpn)));
    Round1Or(done == 0).run(&ps)
}

fn describe(s: &Settings, p: &Player, done: usize) -> P {
    let mut colours = Vec::new();
    let mut opponents = Vec::new();
    for r in &p.history {
        match *r {
            Round::Played {
                opponent, colour, ..
            } => {
                colours.push(colour);
                opponents.push(opponent);
            }
            Round::PairingBye { .. } => colours.push(Colour::White),
            _ => {}
        }
    }
    let score: u32 = p.history.iter().map(|r| r.points()).sum();
    P {
        tpn: p.tpn,
        score,
        pref: preference(&colours),
        opponents,
        colours,
        top: done as u32 + 1 == s.rounds && 2 * score > s.win * done as u32,
    }
}

/// The supplementary colour rules: strong after a difference of two or the
/// same colour twice running, weak otherwise; none before the first game.
fn preference(c: &[Colour]) -> Option<(Colour, bool)> {
    let last = *c.last()?;
    let w = c.iter().filter(|&&x| x == Colour::White).count() as i32;
    let diff = 2 * w - c.len() as i32;
    let fewer = if diff > 0 {
        Colour::Black
    } else {
        Colour::White
    };
    Some(if diff.abs() >= 2 {
        (fewer, true)
    } else if c.len() >= 2 && c[c.len() - 2] == last {
        (last.opposite(), true)
    } else if diff != 0 {
        (fewer, false)
    } else {
        (last.opposite(), false)
    })
}

/// Whether this is round 1, whose colours follow 7.1.
struct Round1Or(bool);

impl Round1Or {
    fn run(&self, ps: &[P]) -> Result<Pairing, Error> {
        let n = ps.len();
        let ctx = Ctx {
            ps,
            first: self.0,
            legal: (0..n)
                .map(|i| (0..n).map(|j| i != j && legal(ps, i, j)).collect())
                .collect(),
        };
        ctx.pair()
    }
}

/// Colour of `x` against `y` from 7.2-7.4 (`x` placed above `y`).
fn colour_of(ps: &[P], x: usize, y: usize) -> Colour {
    let (a, b) = (&ps[x], &ps[y]);
    let whites = |c: &[Colour]| c.iter().filter(|&&k| k == Colour::White).count();
    if whites(&a.colours) != whites(&b.colours) {
        return if whites(&a.colours) < whites(&b.colours) {
            Colour::White
        } else {
            Colour::Black
        };
    }
    for k in 1..=a.colours.len().max(b.colours.len()) {
        let tail = |c: &[Colour]| whites(&c[c.len().saturating_sub(k)..]);
        let (ta, tb) = (tail(&a.colours), tail(&b.colours));
        if ta != tb {
            return if ta < tb {
                Colour::White
            } else {
                Colour::Black
            };
        }
    }
    // 7.4: more points, or the stronger, takes the opposite of his last colour.
    let a_decides = a.score > b.score || (a.score == b.score && a.tpn < b.tpn);
    let decider = if a_decides { a } else { b };
    let mine = decider
        .colours
        .last()
        .map_or(Colour::White, |c| c.opposite());
    if a_decides { mine } else { mine.opposite() }
}

/// 1.2 and the strong colour preferences.
fn legal(ps: &[P], x: usize, y: usize) -> bool {
    let (a, b) = (&ps[x], &ps[y]);
    if a.opponents.contains(&b.tpn) || b.opponents.contains(&a.tpn) {
        return false;
    }
    if a.tpn == X || b.tpn == X || (a.top && b.top) {
        return true;
    }
    let (hi, lo) = if x < y { (x, y) } else { (y, x) };
    let c = colour_of(ps, hi, lo);
    let ok = |p: &P, got: Colour| !matches!(p.pref, Some((want, true)) if want != got);
    ok(&ps[hi], c) && ok(&ps[lo], c.opposite())
}

struct Ctx<'a> {
    ps: &'a [P],
    first: bool,
    legal: Vec<Vec<bool>>,
}

/// One way to pair a pool.
struct Choice {
    pairs: Vec<(usize, usize)>,
    floaters: Vec<usize>,
    natural: bool,
    exchange: Option<(Vec<usize>, Vec<usize>)>,
    /// Colour preferences met.
    met: u32,
}

impl Ctx<'_> {
    fn pair(&self) -> Result<Pairing, Error> {
        let n = self.ps.len();
        let mut free = vec![true; n];
        if !self.completes(&free) {
            return Err(Error::NoLegalPairing);
        }
        let mut all: Vec<(usize, usize)> = Vec::new();
        let mut brackets: Vec<Bracket> = Vec::new();
        let mut carry: Vec<usize> = Vec::new();
        let mut colour_in_b: HashMap<usize, Colour> = HashMap::new();
        let tpn = |i: usize| self.ps[i].tpn;
        let real = |v: &[(usize, usize)]| -> Vec<(u32, u32)> {
            v.iter()
                .filter(|&&(a, b)| tpn(a) != X && tpn(b) != X)
                .map(|&(a, b)| (tpn(a), tpn(b)))
                .collect()
        };
        let mut pos = 0;
        while pos < n {
            let score = self.ps[pos].score;
            let end = (pos..n).find(|&i| self.ps[i].score != score).unwrap_or(n);
            let moved = std::mem::take(&mut carry);
            let mut pairs = self.pair_floaters(&moved, &mut free)?;
            let pool: Vec<usize> = (pos..end).filter(|&i| free[i]).collect();
            let choice = self.pair_pool(&pool, &mut free, &mut colour_in_b)?;
            pairs.extend(choice.pairs.iter().copied());
            all.extend(pairs.iter().copied());
            let wanting = choice
                .pairs
                .iter()
                .filter(|&&(a, b)| tpn(a) != X && tpn(b) != X)
                .flat_map(|&(a, b)| [a, b])
                .filter(|&i| self.ps[i].pref.is_some())
                .count() as u32;
            let unmet = wanting - choice.met;
            brackets.push(Bracket {
                rule: Rule::Nordic,
                score,
                residents: (pos..end).map(tpn).filter(|&t| t != X).collect(),
                moved_down: moved.iter().map(|&i| tpn(i)).collect(),
                pairs: real(&pairs),
                floaters: choice.floaters.iter().map(|&i| tpn(i)).collect(),
                natural: choice.natural && moved.is_empty(),
                exchange: choice.exchange.map(|(x, y)| {
                    (
                        x.iter().map(|&i| tpn(i)).collect(),
                        y.iter().map(|&i| tpn(i)).collect(),
                    )
                }),
                failures: if self.first || unmet == 0 {
                    Vec::new()
                } else {
                    vec![(Criterion::ColourPreference, unmet)]
                },
            });
            carry = choice.floaters;
            pos = end;
        }
        // 9.3: floaters of the lowest pool are paired among themselves.
        let last = self.pair_floaters(&carry, &mut free)?;
        if let Some(b) = brackets.last_mut() {
            b.pairs.extend(real(&last));
        }
        all.extend(last);
        let mut boards = Vec::new();
        let mut bye = None;
        for &(x, y) in &all {
            let (a, b) = (&self.ps[x], &self.ps[y]);
            if b.tpn == X {
                bye = Some(a.tpn);
                continue;
            }
            let c = match (colour_in_b.get(&x), colour_in_b.get(&y)) {
                (_, Some(&cb)) if self.first => cb.opposite(),
                (Some(&ca), _) if self.first => ca,
                _ => colour_of(self.ps, x, y),
            };
            boards.push(if c == Colour::White {
                Board {
                    white: a.tpn,
                    black: b.tpn,
                }
            } else {
                Board {
                    white: b.tpn,
                    black: a.tpn,
                }
            });
        }
        Ok(Pairing {
            boards,
            bye,
            brackets,
        })
    }

    /// 5.4: each floater, in order, meets the nearest lower player not yet
    /// met whose pairing still lets everyone be paired.
    fn pair_floaters(
        &self,
        floaters: &[usize],
        free: &mut [bool],
    ) -> Result<Vec<(usize, usize)>, Error> {
        let n = free.len();
        let mut pairs = Vec::new();
        for &f in floaters {
            if !free[f] {
                continue;
            }
            free[f] = false;
            let g = (f + 1..n)
                .chain(0..f)
                .find(|&g| {
                    free[g] && self.legal[f][g] && {
                        free[g] = false;
                        let ok = self.completes(free);
                        free[g] = true;
                        ok
                    }
                })
                .ok_or(Error::NoLegalPairing)?;
            free[g] = false;
            pairs.push((f.min(g), f.max(g)));
        }
        Ok(pairs)
    }

    /// Can every free player still be paired?
    fn completes(&self, free: &[bool]) -> bool {
        let left: Vec<usize> = (0..free.len()).filter(|&i| free[i]).collect();
        if left.len() % 2 == 1 {
            return false;
        }
        let edges: Vec<(usize, usize, i64)> = (0..left.len())
            .flat_map(|x| (x + 1..left.len()).map(move |y| (x, y)))
            .filter(|&(x, y)| self.legal[left[x]][left[y]])
            .map(|(x, y)| (x, y, 1))
            .collect();
        max_weight_matching(left.len(), &edges)
            .iter()
            .all(|m| m.is_some())
    }

    /// Pairs a pool (players in start-number order), marking the paired ones.
    fn pair_pool(
        &self,
        pool: &[usize],
        free: &mut [bool],
        colour_in_b: &mut HashMap<usize, Colour>,
    ) -> Result<Choice, Error> {
        for (k, floaters) in floater_sets(pool).into_iter().enumerate() {
            let rest: Vec<usize> = pool
                .iter()
                .copied()
                .filter(|i| !floaters.contains(i))
                .collect();
            let half = rest.len() / 2;
            let (a, b) = rest.split_at(half);
            for (e, (a, b, exchange)) in arrangements(a, b).into_iter().enumerate() {
                if let Some((pairs, met)) = self.best(&a, &b, free) {
                    for &(x, y) in &pairs {
                        free[x] = false;
                        free[y] = false;
                    }
                    if self.first {
                        // 7.1: the weakest real player of b has White, then alternately up.
                        let real: Vec<usize> =
                            b.iter().copied().filter(|&i| self.ps[i].tpn != X).collect();
                        for (j, &i) in real.iter().rev().enumerate() {
                            let c = if j % 2 == 0 {
                                Colour::White
                            } else {
                                Colour::Black
                            };
                            colour_in_b.insert(i, c);
                        }
                    }
                    let natural = k == 0
                        && e == 0
                        && pairs
                            .iter()
                            .zip(a.iter().zip(&b))
                            .all(|(&p, (&x, &y))| p == (x.min(y), x.max(y)));
                    return Ok(Choice {
                        pairs,
                        floaters,
                        natural,
                        exchange,
                        met,
                    });
                }
            }
        }
        Err(Error::NoLegalPairing)
    }

    /// The first pairing of `a` against `b` in the order of 5.3 (each `a`
    /// player, from the top, takes the highest `b` player still possible)
    /// that leaves the rest pairable, with the colour preferences it meets.
    fn best(
        &self,
        a: &[usize],
        b: &[usize],
        free: &mut [bool],
    ) -> Option<(Vec<(usize, usize)>, u32)> {
        fn go(
            ctx: &Ctx,
            a: &[usize],
            b: &[usize],
            used: &mut [bool],
            cur: &mut Vec<(usize, usize)>,
            free: &mut [bool],
        ) -> bool {
            let Some(&x) = a.get(cur.len()) else {
                return ctx.completes(free);
            };
            for (j, &y) in b.iter().enumerate() {
                if used[j] || !ctx.legal[x][y] {
                    continue;
                }
                used[j] = true;
                free[x] = false;
                free[y] = false;
                cur.push((x.min(y), x.max(y)));
                if go(ctx, a, b, used, cur, free) {
                    return true;
                }
                cur.pop();
                free[x] = true;
                free[y] = true;
                used[j] = false;
            }
            false
        }
        let mut cur = Vec::new();
        let found = go(self, a, b, &mut vec![false; b.len()], &mut cur, free);
        // Leave `free` as it was; the caller marks the pairs.
        for &(x, y) in &cur {
            free[x] = true;
            free[y] = true;
        }
        found.then(|| {
            let met = cur.iter().map(|&(x, y)| self.met(x, y)).sum();
            (cur, met)
        })
    }

    /// Colour preferences a pair meets.
    fn met(&self, x: usize, y: usize) -> u32 {
        if self.first || self.ps[x].tpn == X || self.ps[y].tpn == X {
            return 0;
        }
        let (hi, lo) = (x.min(y), x.max(y));
        let c = colour_of(self.ps, hi, lo);
        let ok = |p: &P, got: Colour| u32::from(p.pref.is_some_and(|(want, _)| want == got));
        ok(&self.ps[hi], c) + ok(&self.ps[lo], c.opposite())
    }
}

/// Floater sets in the order of 6.1-6.3: the middle player (odd pool) or
/// none (even pool), then the two middle players; then each strongest floater
/// swapped upward, each weakest swapped downward; then the nearest players
/// above and below added; at last the whole pool.
fn floater_sets(pool: &[usize]) -> Vec<Vec<usize>> {
    let n = pool.len();
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut push = |v: Vec<usize>| {
        let mut v = v;
        v.sort_unstable();
        if !out.contains(&v) {
            out.push(v);
        }
    };
    let mut lo = n / 2;
    let even = n.is_multiple_of(2);
    let mut hi = if even { lo } else { lo + 1 };
    if even {
        push(Vec::new());
        if n >= 2 {
            lo -= 1;
            hi += 1;
        }
    }
    while hi <= n && lo < hi {
        let base: Vec<usize> = (lo..hi).collect();
        push(base.iter().map(|&i| pool[i]).collect());
        // The strongest floater swapped upward, then the next.
        for f in 0..base.len() {
            for up in (0..lo).rev() {
                let mut v = base.clone();
                v[f] = up;
                push(v.iter().map(|&i| pool[i]).collect());
            }
        }
        // The weakest floater swapped downward, then the next.
        for f in (0..base.len()).rev() {
            for down in hi..n {
                let mut v = base.clone();
                v[f] = down;
                push(v.iter().map(|&i| pool[i]).collect());
            }
        }
        if lo == 0 || hi == n {
            break;
        }
        lo -= 1;
        hi += 1;
    }
    push(pool.to_vec());
    out
}

/// Arrangements of the halves in the order of 5.3: as they are; then the
/// lowest of `a` changed with the top of `b`, the next of `b`, and so on,
/// then the next lowest of `a`; then two players of each.
type Arrangement = (Vec<usize>, Vec<usize>, Option<(Vec<usize>, Vec<usize>)>);

fn arrangements(a: &[usize], b: &[usize]) -> Vec<Arrangement> {
    let mut out = vec![(a.to_vec(), b.to_vec(), None)];
    let swap = |xs: &[usize], ys: &[usize]| -> Arrangement {
        // Moved b players join a as its lowest; moved a players head b.
        let mut na: Vec<usize> = a.iter().copied().filter(|i| !xs.contains(i)).collect();
        na.extend(ys);
        let mut nb: Vec<usize> = xs.to_vec();
        nb.extend(b.iter().copied().filter(|i| !ys.contains(i)));
        (na, nb, Some((xs.to_vec(), ys.to_vec())))
    };
    for i in (0..a.len()).rev() {
        for &y in b {
            out.push(swap(&[a[i]], &[y]));
        }
    }
    for i in (0..a.len()).rev() {
        for i2 in (0..i).rev() {
            for j in 0..b.len() {
                for j2 in j + 1..b.len() {
                    out.push(swap(&[a[i2], a[i]], &[b[j], b[j2]]));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Players A, B, C, ... = start numbers 1, 2, 3, ... with the given
    /// scores and earlier opponents; colours do not matter here.
    fn round(spec: &[(u32, &[u32])]) -> Vec<P> {
        let mut ps: Vec<P> = spec
            .iter()
            .enumerate()
            .map(|(k, &(score, met))| P {
                tpn: k as u32 + 1,
                score,
                opponents: met.to_vec(),
                colours: Vec::new(),
                pref: None,
                top: false,
            })
            .collect();
        ps.sort_by(|a, b| b.score.cmp(&a.score).then(a.tpn.cmp(&b.tpn)));
        ps
    }

    fn pairs(ps: &[P]) -> Vec<(u32, u32)> {
        let p = Round1Or(false).run(ps).unwrap();
        let mut v: Vec<(u32, u32)> = p
            .boards
            .iter()
            .map(|b| (b.white.min(b.black), b.white.max(b.black)))
            .collect();
        v.sort_unstable();
        v
    }

    // Letters of the guide: A=1, B=2, ... H=8.
    const A: u32 = 1;
    const B: u32 = 2;
    const C: u32 = 3;
    const D: u32 = 4;
    const E: u32 = 5;
    const F: u32 = 6;
    const G: u32 = 7;
    const H: u32 = 8;

    #[test]
    fn guide_odd_pool_middle_player_floats() {
        // 4.9.4: 4½: A B C(F,G) D E; 4: F(C) G(C) H. A-D, B-E, C-H, F-G.
        let ps = round(&[
            (9, &[]),
            (9, &[]),
            (9, &[F, G]),
            (9, &[]),
            (9, &[]),
            (8, &[C]),
            (8, &[C]),
            (8, &[]),
        ]);
        assert_eq!(pairs(&ps), [(A, D), (B, E), (C, H), (F, G)]);
    }

    #[test]
    fn guide_examples_1_to_3_move_b_up_and_undo_pairs() {
        // Example 1: A-E, B-H, C-F, D-G.
        let ps = round(&[
            (2, &[]),
            (2, &[F, G]),
            (2, &[]),
            (2, &[]),
            (2, &[]),
            (2, &[B]),
            (2, &[B]),
            (2, &[]),
        ]);
        assert_eq!(pairs(&ps), [(A, E), (B, H), (C, F), (D, G)]);
        // Example 2: D has also met G: A-E, B-H, C-G, D-F.
        let ps = round(&[
            (2, &[]),
            (2, &[F, G]),
            (2, &[]),
            (2, &[G]),
            (2, &[]),
            (2, &[B]),
            (2, &[B, D]),
            (2, &[]),
        ]);
        assert_eq!(pairs(&ps), [(A, E), (B, H), (C, G), (D, F)]);
        // Example 3: D has met F and G, C has met F: A-F, B-E, C-G, D-H.
        let ps = round(&[
            (2, &[]),
            (2, &[F, G]),
            (2, &[F]),
            (2, &[F, G]),
            (2, &[]),
            (2, &[C, D]),
            (2, &[B, D]),
            (2, &[]),
        ]);
        assert_eq!(pairs(&ps), [(A, F), (B, E), (C, G), (D, H)]);
    }

    #[test]
    fn guide_example_4_moves_a_player_between_the_halves() {
        // A(D,E) B(D) C(D,E) | D(A,B,C) E(A,C) F: C and D change halves.
        let ps = round(&[
            (2, &[D, E]),
            (2, &[D]),
            (2, &[D, E]),
            (2, &[A, B, C]),
            (2, &[A, C]),
            (2, &[]),
        ]);
        assert_eq!(pairs(&ps), [(A, C), (B, E), (D, F)]);
    }

    #[test]
    fn guide_example_7_undoes_a_floaters_pair() {
        // 1½: A B C; 1: D E(F); ½: F(E). A-C, B-E, D-F.
        let ps = round(&[(3, &[]), (3, &[]), (3, &[]), (2, &[]), (2, &[F]), (1, &[E])]);
        assert_eq!(pairs(&ps), [(A, C), (B, E), (D, F)]);
    }

    #[test]
    fn guide_example_8_floater_goes_further_down() {
        // F has also met D: A-C, B-F, D-E.
        let ps = round(&[
            (3, &[]),
            (3, &[]),
            (3, &[]),
            (2, &[F]),
            (2, &[F]),
            (1, &[D, E]),
        ]);
        assert_eq!(pairs(&ps), [(A, C), (B, F), (D, E)]);
    }

    #[test]
    fn guide_example_9_chooses_an_alternative_floater() {
        // F has also met B: B swaps with A as floater; B-C, A-F, D-E.
        let ps = round(&[
            (3, &[]),
            (3, &[F]),
            (3, &[]),
            (2, &[F]),
            (2, &[F]),
            (1, &[B, D, E]),
        ]);
        assert_eq!(pairs(&ps), [(A, F), (B, C), (D, E)]);
    }

    #[test]
    fn colour_examples_of_the_guide() {
        // 4.9.4, p. 4-50; histories earliest first, S = Black, H = White.
        let seq = |s: &str| -> Vec<Colour> {
            s.chars()
                .map(|c| {
                    if c == 'H' {
                        Colour::White
                    } else {
                        Colour::Black
                    }
                })
                .collect()
        };
        let p = |tpn: u32, s: &str| P {
            tpn,
            score: 0,
            opponents: Vec::new(),
            colours: seq(s),
            pref: None,
            top: false,
        };
        let cases = [
            ("SHSHS", "HSHSH", Colour::White),
            ("HHSHS", "HSHSH", Colour::White),
            ("HSSHH", "HSHSH", Colour::Black),
            ("HHSSH", "HSHSH", Colour::White),
            ("SHHSH", "HSHSH", Colour::Black),
            ("HSHSH", "HSHSH", Colour::Black),
        ];
        for (k, (x, y, want)) in cases.into_iter().enumerate() {
            let ps = [p(1, x), p(2, y)];
            assert_eq!(colour_of(&ps, 0, 1), want, "pair {}", k + 1);
        }
    }
}
