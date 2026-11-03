//! Ur solved: the exact chance to win of every position, by value iteration.
//!
//! **What is solved.** For each position before a throw, the chance that the side to throw
//! wins if both sides play perfectly from there on. A throw's best move is then the one that
//! leaves the highest chance. This is what the RoyalUr.net team did for Finkel's, Masters' and
//! Blitz rules (2025); it is done here for every rule set this crate has.
//!
//! **Every position has a number.** [`Space`] ranks positions without gaps, so the table is a
//! flat array and a position's place in it is computed, never searched. A position is written
//! from the side to throw: its pieces (`me`) and the other side's (`them`), each a mask of the
//! steps along its own path, and how many each has home. The two paths are mirror images in
//! every rule set (checked in [`Space::new`]), so a position with Black to throw has the same
//! number as its mirror with White to throw, and only one side's view is stored.
//!
//! The rank is, from the outside in:
//! - the layer: how many pieces each side has home (`mo`, `to`, each below the rule set's
//!   pieces; a side with all home has won, and its positions are not stored);
//! - how many pieces each side has on the shared squares (`kme`, `kth`);
//! - the pattern of the shared squares, ranked among the patterns with those counts;
//! - each side's private squares, ranked among the masks with few enough pieces.
//!
//! **Layers are solved from the end back.** A piece home never leaves home, so a move only
//! keeps the layer or moves to one with more pieces home. The layers `(a, b)` and `(b, a)` feed
//! each other (the turn passes between them) and are solved together, by sweeps of
//! Gauss–Seidel iteration until no value moves by more than [`EPSILON`], after every layer they
//! lead to is done. Sweeps run on as many threads as asked for, each taking patterns in turn.
//!
//! No dependencies: threads and atomics are the standard library's, so this stays a rules
//! crate.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crate::board::{Rules, Side, Spot};
use crate::dice::Throw;
use crate::position::{Move, Position};

/// The most steps on a path, and one more for step 0.
const STEPS: usize = 17;

/// A layer group is done when no value moved by more than this in a sweep.
pub const EPSILON: f32 = 1e-7;

/// The ranked space of positions of one rule set.
pub struct Space {
    pub rules: &'static Rules,
    len: u8,
    pieces: u8,
    shared: u8,
    /// For a step of mine, the other side's step on the same square; 0 for none.
    cross: [u8; STEPS],
    /// My steps that earn another throw, and those where no piece can be taken.
    rosette: u32,
    safe: u32,
    /// A step mask (bits 1..=len, shifted down by one) split into its shared-square bits and
    /// its private bits, from my side and from the other side.
    me_split: Vec<(u16, u16)>,
    them_split: Vec<(u16, u16)>,
    /// Back again: shared bits to my steps, to theirs, and private bits to steps.
    me_join: Vec<u32>,
    them_join: Vec<u32>,
    private_join: Vec<u32>,
    /// A private mask's rank among them all ordered by how many pieces they hold, and back.
    prank: Vec<u32>,
    plist: Vec<u16>,
    /// How many private masks hold this many pieces or fewer.
    cnt_le: [u64; STEPS],
    choose: [[u64; STEPS]; STEPS],
    /// Where each layer starts, and each block of a layer: `[layer][kme][kth]`.
    base: Vec<u64>,
    block: Vec<u64>,
    total: u64,
    /// Each throw's share of the dice's falls, and its steps.
    throws: Vec<(f64, u8)>,
}

/// What a solve did: its size and its work, counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Positions in the table.
    pub states: u64,
    /// Positions worked out, counting each sweep.
    pub updates: u64,
    /// Moves looked at.
    pub moves: u64,
    /// Sweeps, over every layer group.
    pub sweeps: u64,
}

/// Win chances by a position's rank: a solved table, in memory or in a file.
pub trait Chances {
    /// The chance the side to throw wins, 0 to 1; None when it cannot be read.
    fn chance(&self, index: u64) -> Option<f64>;
}

/// A table being solved or just solved: each value an `f32`'s bits.
pub struct Solved {
    values: Vec<AtomicU32>,
}

impl Solved {
    pub fn len(&self) -> u64 {
        self.values.len() as u64
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn get(&self, index: u64) -> f32 {
        f32::from_bits(self.values[index as usize].load(Ordering::Relaxed))
    }
}

impl Chances for Solved {
    fn chance(&self, index: u64) -> Option<f64> {
        (index < self.len()).then(|| self.get(index) as f64)
    }
}

fn bit(step: u8) -> u32 {
    1 << step
}

impl Space {
    /// The space of `rules`. Panics if its two paths are not mirror images, which no rule set
    /// of this crate allows (a test builds every one).
    pub fn new(rules: &'static Rules) -> Space {
        let len = rules.path_len();
        let square = |side: Side, step: u8| rules.square_at(side, Spot::Step(step)).expect("a step on the path");
        let cross_of = |side: Side, step: u8| (1..=len).find(|&t| square(side.other(), t) == square(side, step)).unwrap_or(0);
        let mut cross = [0u8; STEPS];
        let (mut rosette, mut safe) = (0u32, 0u32);
        for s in 1..=len {
            cross[s as usize] = cross_of(Side::White, s);
            let (w, b) = (square(Side::White, s), square(Side::Black, s));
            assert!(
                cross_of(Side::Black, s) == cross[s as usize] && rules.is_rosette(w) == rules.is_rosette(b) && rules.is_safe(w) == rules.is_safe(b),
                "{}: the two paths are not mirror images",
                rules.name
            );
            if rules.is_rosette(w) {
                rosette |= bit(s);
            }
            if rules.is_safe(w) {
                safe |= bit(s);
            }
        }
        let shared_steps: Vec<u8> = (1..=len).filter(|&s| cross[s as usize] != 0).collect();
        let private_steps: Vec<u8> = (1..=len).filter(|&s| cross[s as usize] == 0).collect();
        let (shared, private) = (shared_steps.len() as u8, private_steps.len() as u8);
        let shared_index = |s: u8| shared_steps.iter().position(|&x| x == s);
        let private_index = |s: u8| private_steps.iter().position(|&x| x == s);

        let masks = 1usize << len;
        let split = |mine: bool| -> Vec<(u16, u16)> {
            (0..masks)
                .map(|m| {
                    let (mut a, mut p) = (0u16, 0u16);
                    for s in 1..=len {
                        if m & (1 << (s - 1)) == 0 {
                            continue;
                        }
                        // Their step `s` is on the square of my step `cross[s]`.
                        let shared_at = if mine { shared_index(s) } else { shared_index(cross[s as usize]) };
                        match (cross[s as usize] != 0, shared_at, private_index(s)) {
                            (true, Some(j), _) => a |= 1 << j,
                            (false, _, Some(j)) => p |= 1 << j,
                            _ => unreachable!(),
                        }
                    }
                    (a, p)
                })
                .collect()
        };
        let me_split = split(true);
        let them_split = split(false);
        let me_join: Vec<u32> = (0..1usize << shared).map(|a| (0..shared).filter(|j| a & (1 << j) != 0).map(|j| bit(shared_steps[j as usize])).sum()).collect();
        let them_join: Vec<u32> = (0..1usize << shared).map(|a| (0..shared).filter(|j| a & (1 << j) != 0).map(|j| bit(cross[shared_steps[j as usize] as usize])).sum()).collect();
        let private_join: Vec<u32> = (0..1usize << private).map(|p| (0..private).filter(|j| p & (1 << j) != 0).map(|j| bit(private_steps[j as usize])).sum()).collect();

        let mut plist: Vec<u16> = (0..1u16 << private).collect();
        plist.sort_by_key(|p| (p.count_ones(), *p));
        let mut prank = vec![0u32; plist.len()];
        for (r, &p) in plist.iter().enumerate() {
            prank[p as usize] = r as u32;
        }
        let mut cnt_le = [0u64; STEPS];
        for (n, c) in cnt_le.iter_mut().enumerate() {
            *c = plist.iter().filter(|p| p.count_ones() as usize <= n).count() as u64;
        }
        let mut choose = [[0u64; STEPS]; STEPS];
        for n in 0..STEPS {
            choose[n][0] = 1;
            for k in 1..=n {
                choose[n][k] = choose[n - 1][k - 1] + if k < n { choose[n - 1][k] } else { 0 };
            }
        }

        let throws = rules.throws().map(|t| (rules.ways(t) as f64 / (1u32 << rules.dice) as f64, rules.steps(t))).collect();
        let mut space = Space {
            rules,
            len,
            pieces: rules.pieces,
            shared,
            cross,
            rosette,
            safe,
            me_split,
            them_split,
            me_join,
            them_join,
            private_join,
            prank,
            plist,
            cnt_le,
            choose,
            base: vec![],
            block: vec![],
            total: 0,
            throws,
        };
        let p = space.pieces as usize;
        let k = shared as usize + 1;
        let mut at = 0u64;
        for mo in 0..p {
            for to in 0..p {
                space.base.push(at);
                let start = at;
                for kme in 0..k {
                    for kth in 0..k {
                        space.block.push(at - start);
                        at += space.ways(shared as usize, kme, kth) * space.fits(mo, kme) * space.fits(to, kth);
                    }
                }
            }
        }
        space.total = at;
        space
    }

    /// Positions in the table.
    pub fn len(&self) -> u64 {
        self.total
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// Patterns of `n` shared squares with `a` of mine and `b` of theirs.
    fn ways(&self, n: usize, a: usize, b: usize) -> u64 {
        if a + b > n { 0 } else { self.choose[n][a] * self.choose[n - a][b] }
    }

    /// How many private masks a side with `home` pieces home and `k` on shared squares can have.
    fn fits(&self, home: usize, k: usize) -> u64 {
        let p = self.pieces as usize;
        if home + k > p { 0 } else { self.cnt_le[(p - home - k).min(STEPS - 1)] }
    }

    fn block_at(&self, mo: usize, to: usize, kme: usize, kth: usize) -> u64 {
        let p = self.pieces as usize;
        let k = self.shared as usize + 1;
        self.base[mo * p + to] + self.block[(mo * p + to) * k * k + kme * k + kth]
    }

    /// The rank of a shared-square pattern among those with as many of each side's pieces.
    fn pattern_rank(&self, a: u16, b: u16) -> u64 {
        let (mut ka, mut kb) = (a.count_ones() as usize, b.count_ones() as usize);
        let mut n = self.shared as usize;
        let mut r = 0;
        for j in 0..self.shared {
            n -= 1;
            if a & (1 << j) != 0 {
                r += self.ways(n, ka, kb);
                ka -= 1;
            } else if b & (1 << j) != 0 {
                r += self.ways(n, ka, kb) + if ka > 0 { self.ways(n, ka - 1, kb) } else { 0 };
                kb -= 1;
            }
        }
        r
    }

    /// The rank of a position written as step masks (bits 1..=len), from the side to throw.
    pub fn rank(&self, me: u32, them: u32, mo: u8, to: u8) -> u64 {
        let (a, p) = self.me_split[(me >> 1) as usize];
        let (b, q) = self.them_split[(them >> 1) as usize];
        let (kme, kth) = (a.count_ones() as usize, b.count_ones() as usize);
        let nth = self.fits(to as usize, kth);
        let n = self.fits(mo as usize, kme) * nth;
        self.block_at(mo as usize, to as usize, kme, kth) + self.pattern_rank(a, b) * n + self.prank[p as usize] as u64 * nth + self.prank[q as usize] as u64
    }

    /// A position's step masks, from the side to throw, and each side's pieces home.
    fn masks(position: &Position) -> (u32, u32, u8, u8) {
        let mover = position.to_move();
        let mask = |side: Side| position.pieces(side).iter().filter_map(|s| match s { Spot::Step(s) => Some(bit(*s)), _ => None }).sum::<u32>();
        (mask(mover), mask(mover.other()), position.off(mover), position.off(mover.other()))
    }

    /// A position's place in the table; None once the game is won.
    pub fn index(&self, position: &Position) -> Option<u64> {
        if position.winner().is_some() || position.rules() != self.rules {
            return None;
        }
        let (me, them, mo, to) = Self::masks(position);
        Some(self.rank(me, them, mo, to))
    }

    /// The chance the side to throw wins, before it throws.
    pub fn before_throw(&self, table: &dyn Chances, position: &Position) -> Option<f64> {
        match position.winner() {
            Some(side) => Some(if side == position.to_move() { 1.0 } else { 0.0 }),
            None => table.chance(self.index(position)?),
        }
    }

    /// The mover's chance after playing `chosen` in `position`.
    pub fn after(&self, table: &dyn Chances, position: &Position, chosen: Move) -> Option<f64> {
        let mover = position.to_move();
        let mut next = position.clone();
        next.play(chosen);
        let chance = self.before_throw(table, &next)?;
        Some(if next.to_move() == mover { chance } else { 1.0 - chance })
    }

    /// Each legal move for `throw` and the mover's chance after it, the best first.
    pub fn moves(&self, table: &dyn Chances, position: &Position, throw: Throw) -> Option<Vec<(Move, f64)>> {
        let mut all = position
            .moves_for(throw)
            .into_iter()
            .map(|o| Some((o.chosen, self.after(table, position, o.chosen)?)))
            .collect::<Option<Vec<_>>>()?;
        all.sort_by(|x, y| y.1.total_cmp(&x.1));
        Some(all)
    }

    /// The value of one position worked out from the table as it stands: the mover's chance,
    /// averaged over the throws, each played as well as the table says.
    fn value(&self, values: &[AtomicU32], me: u32, them: u32, mo: u8, to: u8, moves: &mut u64) -> f32 {
        let get = |me: u32, them: u32, mo: u8, to: u8| f32::from_bits(values[self.rank(me, them, mo, to) as usize].load(Ordering::Relaxed)) as f64;
        let waiting = self.pieces - mo - me.count_ones() as u8;
        let end = self.len + 1;
        let mut total = 0.0f64;
        for &(share, steps) in &self.throws {
            let mut best = -1.0f64;
            if steps > 0 {
                let mut from_mask = me;
                let mut from = if waiting > 0 { 0 } else { from_mask.trailing_zeros() as u8 };
                while from < 32 {
                    *moves += 1;
                    let dest = from + steps;
                    let lifted = me & !bit(from);
                    let v = if dest == end {
                        if mo + 1 == self.pieces { 1.0 } else { 1.0 - get(them, lifted, to, mo + 1) }
                    } else if dest < end && me & bit(dest) == 0 {
                        let c = self.cross[dest as usize];
                        let captures = c != 0 && them & bit(c) != 0;
                        if captures && self.safe & bit(dest) != 0 {
                            -1.0
                        } else {
                            let them2 = if captures { them & !bit(c) } else { them };
                            let me2 = lifted | bit(dest);
                            if self.rosette & bit(dest) != 0 || (captures && self.rules.capture_again) {
                                get(me2, them2, mo, to)
                            } else {
                                1.0 - get(them2, me2, to, mo)
                            }
                        }
                    } else {
                        -1.0
                    };
                    best = best.max(v);
                    from_mask &= !bit(from);
                    from = if from_mask == 0 { 32 } else { from_mask.trailing_zeros() as u8 };
                }
            }
            if best < 0.0 {
                best = 1.0 - get(them, me, to, mo);
            }
            total += share * best;
        }
        total as f32
    }

    /// Solves the whole table on `threads` threads. `stop` is asked between sweeps; `progress`
    /// hears the positions done and in all, after each layer group. None when stopped.
    pub fn solve(&self, threads: usize, stop: &(dyn Fn() -> bool + Sync), progress: &(dyn Fn(u64, u64) + Sync)) -> Option<(Solved, Stats)> {
        let values: Vec<AtomicU32> = (0..self.total).map(|_| AtomicU32::new(0.5f32.to_bits())).collect();
        let p = self.pieces as usize;
        let k = self.shared as usize + 1;
        // Every shared pattern, by its counts, in rank order.
        let mut patterns: Vec<Vec<(u16, u16)>> = (0..k * k).map(|i| vec![(0, 0); self.ways(self.shared as usize, i / k, i % k) as usize]).collect();
        for a in 0..1u16 << self.shared {
            let rest = !a & ((1u16 << self.shared) - 1);
            let mut b = rest;
            loop {
                let slot = a.count_ones() as usize * k + b.count_ones() as usize;
                patterns[slot][self.pattern_rank(a, b) as usize] = (a, b);
                if b == 0 {
                    break;
                }
                b = (b - 1) & rest;
            }
        }
        let mut stats = Stats { states: self.total, ..Stats::default() };
        let mut done = 0u64;
        let threads = threads.max(1);
        for sum in (0..=2 * (p - 1)).rev() {
            for a in 0..p {
                let Some(b) = sum.checked_sub(a).filter(|&b| b < p && a <= b) else { continue };
                let layers: Vec<(usize, usize)> = if a == b { vec![(a, b)] } else { vec![(a, b), (b, a)] };
                // The work of one sweep: (layer, block, first pattern, patterns).
                let mut items: Vec<(usize, usize, usize, usize)> = vec![];
                let mut size = 0;
                for &(mo, to) in &layers {
                    for kme in 0..k {
                        for kth in 0..k {
                            let n = self.fits(mo, kme) * self.fits(to, kth);
                            let count = patterns[kme * k + kth].len();
                            if n == 0 || count == 0 {
                                continue;
                            }
                            size += n * count as u64;
                            let chunk = (4096 / n as usize).max(1);
                            items.extend((0..count).step_by(chunk).map(|f| (mo * p + to, kme * k + kth, f, chunk.min(count - f))));
                        }
                    }
                }
                loop {
                    if stop() {
                        return None;
                    }
                    let next = AtomicUsize::new(0);
                    let worst = AtomicU32::new(0);
                    let (updates, moves) = (AtomicU64::new(0), AtomicU64::new(0));
                    let moved = AtomicBool::new(false);
                    std::thread::scope(|scope| {
                        for _ in 0..threads {
                            scope.spawn(|| {
                                let (mut u, mut m, mut w) = (0u64, 0u64, 0f32);
                                while let Some(&(layer, slot, first, count)) = items.get(next.fetch_add(1, Ordering::Relaxed)) {
                                    let (mo, to) = ((layer / p) as u8, (layer % p) as u8);
                                    let (kme, kth) = (slot / k, slot % k);
                                    let (nme, nth) = (self.fits(mo as usize, kme), self.fits(to as usize, kth));
                                    let start = self.block_at(mo as usize, to as usize, kme, kth);
                                    for (r, &(sa, sb)) in patterns[slot].iter().enumerate().skip(first).take(count) {
                                        let (me_s, them_s) = (self.me_join[sa as usize], self.them_join[sb as usize]);
                                        let mut at = start + r as u64 * nme * nth;
                                        for rp in 0..nme as usize {
                                            let me = me_s | self.private_join[self.plist[rp] as usize];
                                            for rq in 0..nth as usize {
                                                let them = them_s | self.private_join[self.plist[rq] as usize];
                                                let v = self.value(&values, me, them, mo, to, &mut m);
                                                let old = f32::from_bits(values[at as usize].swap(v.to_bits(), Ordering::Relaxed));
                                                w = w.max((v - old).abs());
                                                u += 1;
                                                at += 1;
                                            }
                                        }
                                    }
                                }
                                worst.fetch_max(w.to_bits(), Ordering::Relaxed);
                                updates.fetch_add(u, Ordering::Relaxed);
                                moves.fetch_add(m, Ordering::Relaxed);
                                moved.store(true, Ordering::Relaxed);
                            });
                        }
                    });
                    debug_assert!(moved.load(Ordering::Relaxed));
                    stats.sweeps += 1;
                    stats.updates += updates.into_inner();
                    stats.moves += moves.into_inner();
                    // Non-negative floats order as their bits do.
                    if f32::from_bits(worst.into_inner()) <= EPSILON {
                        break;
                    }
                }
                done += size;
                progress(done, self.total);
            }
        }
        Some((Solved { values }, stats))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{ALL, BLITZ, FINKEL, MASTERS};
    use std::collections::HashMap;

    /// Finkel's board with two pieces a side: small enough to solve both ways in a test.
    static TINY: Rules = Rules { name: "tiny", pieces: 2, ..FINKEL };
    /// Masters' path with two pieces a side and a capture that throws again.
    static TINY_BLITZ: Rules = Rules { name: "tiny-blitz", pieces: 2, ..BLITZ };
    static TINY_MASTERS: Rules = Rules { name: "tiny-masters", pieces: 2, ..MASTERS };

    /// The counts of positions where neither side has won that the solver of the RoyalUr.net
    /// team reports for Finkel and Blitz (J. Olieslagers, `game_of_ur`, 2025).
    #[test]
    fn the_spaces_have_the_published_sizes() {
        assert_eq!(Space::new(&FINKEL).len(), 137_870_097);
        assert_eq!(Space::new(&BLITZ).len(), 41_243_781);
        // Theirs for Masters, 500,981,472, counts the won positions too.
        assert_eq!(Space::new(&MASTERS).len(), 500_929_993);
        for rules in ALL {
            assert!(!Space::new(rules).is_empty(), "{}", rules.name);
        }
    }

    /// Every position of a small space has its own rank, and they fill the table.
    #[test]
    fn ranks_fill_the_table_without_gaps() {
        for rules in [&TINY, &TINY_BLITZ, &TINY_MASTERS] {
            let space = Space::new(rules);
            let mut seen = vec![false; space.len() as usize];
            let len = rules.path_len();
            for me in (0..1u32 << len).map(|m| m << 1).filter(|m| m.count_ones() <= 2) {
                for them in (0..1u32 << len).map(|m| m << 1).filter(|m| m.count_ones() <= 2) {
                    // Two pieces on one square cannot be.
                    let clash = (1..=len).any(|s| me & bit(s) != 0 && space.cross[s as usize] != 0 && them & bit(space.cross[s as usize]) != 0);
                    for mo in 0..2u8 {
                        for to in 0..2u8 {
                            if clash || me.count_ones() as u8 + mo > 2 || them.count_ones() as u8 + to > 2 {
                                continue;
                            }
                            let r = space.rank(me, them, mo, to) as usize;
                            assert!(!seen[r], "{}: rank {r} twice", rules.name);
                            seen[r] = true;
                        }
                    }
                }
            }
            assert!(seen.iter().all(|&s| s), "{}: a rank with no position", rules.name);
        }
    }

    /// The same chances worked out the slow way: every position reached from the start, by
    /// the crate's own moves, iterated until nothing moves.
    fn naive(rules: &'static Rules) -> HashMap<String, f64> {
        let start = Position::starting_with(rules, Side::White);
        let mut all = vec![start.clone()];
        let mut seen: HashMap<String, usize> = HashMap::from([(start.to_picture(), 0)]);
        // For each position, each throw's share and where its moves lead (and whether the mover moves again).
        type Throws = Vec<(f64, Vec<(usize, bool)>)>;
        let mut next: Vec<Throws> = vec![];
        let mut i = 0;
        while i < all.len() {
            let position = all[i].clone();
            let mut throws = vec![];
            for throw in rules.throws() {
                let share = rules.ways(throw) as f64 / (1u32 << rules.dice) as f64;
                let mut options = vec![];
                let offers = position.moves_for(throw);
                let afters: Vec<Position> = if offers.is_empty() {
                    let mut p = position.clone();
                    p.pass();
                    vec![p]
                } else {
                    offers.iter().map(|o| { let mut p = position.clone(); p.play(o.chosen); p }).collect()
                };
                for after in afters {
                    let same = after.to_move() == position.to_move();
                    let picture = after.to_picture();
                    let at = *seen.entry(picture).or_insert_with(|| { all.push(after.clone()); all.len() - 1 });
                    options.push((at, same));
                }
                throws.push((share, options));
            }
            next.push(throws);
            i += 1;
        }
        let mut value: Vec<f64> = all.iter().map(|p| match p.winner() { Some(w) if w == p.to_move() => 1.0, Some(_) => 0.0, None => 0.5 }).collect();
        loop {
            let mut worst = 0.0f64;
            for (i, throws) in next.iter().enumerate() {
                if all[i].winner().is_some() {
                    continue;
                }
                let v: f64 = throws.iter().map(|(share, options)| share * options.iter().map(|&(at, same)| if same { value[at] } else { 1.0 - value[at] }).fold(-1.0, f64::max)).sum();
                worst = worst.max((v - value[i]).abs());
                value[i] = v;
            }
            if worst < 1e-12 {
                break;
            }
        }
        all.iter().map(|p| p.to_picture()).zip(value).collect()
    }

    #[test]
    fn a_small_rule_set_solves_as_its_moves_say() {
        for rules in [&TINY, &TINY_BLITZ, &TINY_MASTERS] {
            let space = Space::new(rules);
            let (solved, stats) = space.solve(4, &|| false, &|_, _| {}).unwrap();
            let slow = naive(rules);
            let mut checked = 0;
            for (picture, chance) in &slow {
                let position = Position::from_picture(rules, picture).unwrap();
                if position.winner().is_some() {
                    continue;
                }
                let fast = space.before_throw(&solved, &position).unwrap();
                assert!((fast - chance).abs() < 1e-5, "{}: {picture}: {fast} against {chance}", rules.name);
                checked += 1;
            }
            assert!(checked > 1000, "{}: {checked}", rules.name);
            // The work, counted: each position is worked out in a handful of sweeps.
            assert!(stats.updates < 60 * stats.states, "{}: {stats:?}", rules.name);
            assert!(stats.moves > stats.updates, "{stats:?}");
        }
    }

    #[test]
    fn the_best_move_comes_first_and_a_win_is_certain() {
        let space = Space::new(&TINY);
        let (solved, _) = space.solve(2, &|| false, &|_, _| {}).unwrap();
        // White's last piece on its last step, Black's both waiting: a 1 wins.
        let mut glyphs = vec!['.'; 20];
        glyphs[4] = 'W';
        let position = Position::from_picture(&TINY, &format!("{} 02 10 w", glyphs.into_iter().collect::<String>())).unwrap();
        let moves = space.moves(&solved, &position, Throw::new(1).unwrap()).unwrap();
        assert_eq!(moves[0].0, Move { from: Spot::Step(14), to: Spot::Off });
        assert_eq!(moves[0].1, 1.0);
    }

    #[test]
    fn a_stopped_solve_gives_nothing() {
        assert!(Space::new(&TINY).solve(1, &|| true, &|_, _| {}).is_none());
    }
}
