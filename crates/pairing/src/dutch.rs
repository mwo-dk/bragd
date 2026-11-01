//! The FIDE (Dutch) Swiss system, C.04.3 as applied from 1 February 2026,
//! with the Basic Rules (C.04.1) and General Handling Rules (C.04.2) of the
//! same date.
//!
//! Brackets are paired from the top scoregroup down (1.9.2). For each bracket
//! the rules ask for the candidate with the best failure values on criteria
//! C5-C21, and among equals the one generated first (3.8). Two steps find it:
//!
//! 1. **The best failure values.** One maximum-weight matching over the bracket,
//!    every player below it and, when the number of players is odd, a bye
//!    vertex. A perfect matching is a completed round-pairing, so C4 holds by
//!    construction. Each criterion is a field of bits in the edge weights, in
//!    the rules' order of priority: the bye vertex carries C5 and C9, a bracket
//!    player matched outside the bracket is a downfloater (C6, C7, C14, C16,
//!    C18, C20), a player of the next bracket matched further down is a
//!    downfloater of that bracket (C8), and a pair inside the bracket carries
//!    its colour and float criteria (C10-C13, C15, C17, C19, C21).
//! 2. **The first candidate with those values.** The generation order of
//!    3.6, 3.7 and 4 is walked choice by choice: the set of MDPs to pair (4.4),
//!    each MDP's opponent (4.2), the remainder's exchange (4.3) and each of its
//!    S1 players' opponents (4.2). At each choice the earliest option is kept
//!    for which the same matching, restricted to that choice, still reaches the
//!    best values.

use crate::colour::{self, Preference, Side, Strength};
use crate::matching::{wide_matching, wide_matching_duals};
use crate::wide::{BITS, Wide};
use crate::{Board, Colour, Error, Player, Round, Settings};
use std::collections::HashMap;

/// A paired round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pairing {
    /// Boards in publishing order (C.04.2 3.6).
    pub boards: Vec<Board>,
    /// The player given the pairing-allocated bye, if the number of players is odd.
    pub bye: Option<u32>,
    /// How each bracket was paired, from the top scoregroup down.
    pub brackets: Vec<Bracket>,
}

/// How one bracket was paired: the facts behind a director's "why".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bracket {
    /// The score of the resident players, in the caller's points.
    pub score: u32,
    /// Resident players, by TPN, in bracket order.
    pub residents: Vec<u32>,
    /// Moved-down players from the bracket above, in bracket order.
    pub moved_down: Vec<u32>,
    /// Pairs made in this bracket, higher ranked player first.
    pub pairs: Vec<(u32, u32)>,
    /// Players who float down to the next bracket (or take the bye).
    pub floaters: Vec<u32>,
    /// True when the pairs are the first candidate: no transposition, no exchange.
    pub natural: bool,
    /// The resident exchange used, as (moved from S1 to S2, moved from S2 to S1), by TPN.
    pub exchange: Option<(Vec<u32>, Vec<u32>)>,
    /// Every quality criterion the chosen candidate does not fully meet, with
    /// its failure count. Empty for a perfect candidate.
    pub failures: Vec<(Criterion, u32)>,
}

/// The criteria of C.04.3 2.3-2.4 a bracket can fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Criterion {
    /// C5: the score of the player who takes the bye.
    ByeScore,
    /// C6: downfloaters from this bracket.
    Downfloaters,
    /// C7: the scores of the downfloaters, highest first.
    DownfloaterScores,
    /// C8: downfloaters this choice forces out of the next bracket.
    NextBracketDownfloaters,
    /// C8: the scores of those downfloaters, highest first.
    NextBracketDownfloaterScores,
    /// C9: unplayed rounds of the player who takes the bye.
    ByeUnplayedRounds,
    /// C10: topscorers or their opponents beyond a colour difference of 2.
    TopscorerColourDifference,
    /// C11: topscorers or their opponents with the same colour three times running.
    TopscorerColourRepeat,
    /// C12: players who do not get their colour preference.
    ColourPreference,
    /// C13: players who do not get their strong colour preference.
    StrongColourPreference,
    /// C14: resident downfloaters who floated down last round.
    RepeatDownfloat,
    /// C15: opponents of moved-down players who floated up last round.
    RepeatUpfloat,
    /// C16: resident downfloaters who floated down two rounds ago.
    RepeatDownfloatTwoRounds,
    /// C17: opponents of moved-down players who floated up two rounds ago.
    RepeatUpfloatTwoRounds,
    /// C18: moved-down players who floated down last round, again.
    RepeatMovedDown,
    /// C19: their opponents who floated up last round.
    RepeatMovedDownOpponent,
    /// C20: moved-down players who floated down two rounds ago, again.
    RepeatMovedDownTwoRounds,
    /// C21: their opponents who floated up two rounds ago.
    RepeatMovedDownOpponentTwoRounds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Float {
    None,
    Down,
    Up,
}

/// A present player as the rules see them.
struct Info {
    tpn: u32,
    score: u32,
    played: Vec<Colour>,
    opponents: Vec<u32>,
    bye_ok: bool,
    /// Floats in the last round and the one before (1.4).
    float: [Float; 2],
    unplayed: u32,
    topscorer: bool,
    pref: Option<Preference>,
}

/// Pairs one round under the Dutch system.
///
/// `players` holds everyone registered, with one history entry per finished
/// round; `present` lists the TPNs to pair this round (withdrawn and absent
/// players are left out).
pub fn pair(settings: &Settings, players: &[Player], present: &[u32]) -> Result<Pairing, Error> {
    let by_tpn = index(players)?;
    let done = players.first().map_or(0, |p| p.history.len());
    if done as u32 >= settings.rounds {
        return Err(Error::TournamentOver);
    }
    let mut info = Vec::with_capacity(present.len());
    for &t in present {
        let p = by_tpn.get(&t).ok_or(Error::UnknownPlayer(t))?;
        info.push(describe(settings, &by_tpn, p, done));
    }
    info.sort_by(|a, b| b.score.cmp(&a.score).then(a.tpn.cmp(&b.tpn)));
    for w in info.windows(2) {
        if w[0].tpn == w[1].tpn {
            return Err(Error::DuplicatePlayer(w[0].tpn));
        }
    }
    let round = Ctx {
        s: settings,
        info: &info,
        levels: score_levels(&info),
    };
    round.pair()
}

/// How a given pairing of a round stands against the rules' own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The same pairs and colours.
    Same,
    /// The same pairs; some colours differ from Article 5.
    Colours(Vec<Board>),
    /// A pair breaks C1 or C3, or the rest of the round cannot be completed (C4).
    Illegal { bracket: u32 },
    /// Legal, but this criterion is met less well than the rules' pairing manages.
    Worse { bracket: u32, criterion: Criterion },
    /// As good on every criterion; the rules take the candidate generated first.
    GeneratedLater { bracket: u32 },
    /// Better on this criterion than the rules' pairing: a fault in this crate.
    Better { bracket: u32, criterion: Criterion },
}

/// Judges a pairing made by hand, or by another program, against the rules.
///
/// Brackets are walked from the top as [`pair`] does. At the first bracket where
/// the given pairs differ, the given choice is scored on every criterion and
/// compared with the rules' choice. `bracket` is that bracket's resident score.
pub fn assess(
    settings: &Settings,
    players: &[Player],
    present: &[u32],
    boards: &[Board],
) -> Result<Verdict, Error> {
    let ours = pair(settings, players, present)?;
    let by_tpn = index(players)?;
    let done = players.first().map_or(0, |p| p.history.len());
    let mut info = Vec::with_capacity(present.len());
    for &t in present {
        let p = by_tpn.get(&t).ok_or(Error::UnknownPlayer(t))?;
        info.push(describe(settings, &by_tpn, p, done));
    }
    info.sort_by(|a, b| b.score.cmp(&a.score).then(a.tpn.cmp(&b.tpn)));
    let ctx = Ctx {
        s: settings,
        info: &info,
        levels: score_levels(&info),
    };
    let at: HashMap<u32, usize> = info.iter().enumerate().map(|(k, i)| (i.tpn, k)).collect();
    let mut given: Vec<Option<usize>> = vec![None; info.len()];
    for b in boards {
        let (Some(&w), Some(&k)) = (at.get(&b.white), at.get(&b.black)) else {
            return Err(Error::UnknownPlayer(if at.contains_key(&b.white) {
                b.black
            } else {
                b.white
            }));
        };
        given[w] = Some(k);
        given[k] = Some(w);
        if !ctx.compatible(w, k) {
            return Ok(Verdict::Illegal {
                bracket: info[w.min(k)].score,
            });
        }
    }
    let n = info.len();
    let mut mdps: Vec<usize> = Vec::new();
    let mut pos = 0;
    while pos < n {
        let score = info[pos].score;
        let end = (pos..n).find(|&i| info[i].score != score).unwrap_or(n);
        let mut b: Vec<usize> = mdps.clone();
        b.extend(pos..end);
        let lower: Vec<usize> = (end..n).collect();
        let solver = BracketSolver::new(&ctx, &b, mdps.len(), &lower)?;
        let solved = solver.solve()?;
        let inside = |x: usize| given[x].is_some_and(|y| b.contains(&y));
        let mut theirs: Vec<(usize, usize)> = b
            .iter()
            .filter(|&&x| inside(x) && given[x].unwrap() > x)
            .map(|&x| (x, given[x].unwrap()))
            .collect();
        let mut mine = solved.pairs.clone();
        theirs.sort_unstable();
        mine.sort_unstable();
        if theirs != mine {
            return Ok(solver.judge(&theirs, &given, score));
        }
        mdps = b.iter().copied().filter(|&x| !inside(x)).collect();
        pos = end;
    }
    let mut differ: Vec<Board> = boards
        .iter()
        .filter(|b| !ours.boards.contains(b))
        .copied()
        .collect();
    differ.sort_by_key(|b| (b.white, b.black));
    Ok(if differ.is_empty() {
        Verdict::Same
    } else {
        Verdict::Colours(differ)
    })
}

fn index(players: &[Player]) -> Result<HashMap<u32, &Player>, Error> {
    let mut m = HashMap::new();
    let len = players.first().map_or(0, |p| p.history.len());
    for p in players {
        if p.history.len() != len {
            return Err(Error::UnevenHistory(p.tpn));
        }
        if m.insert(p.tpn, p).is_some() {
            return Err(Error::DuplicatePlayer(p.tpn));
        }
    }
    Ok(m)
}

fn score_before(p: &Player, round: usize) -> u32 {
    p.history[..round].iter().map(|r| r.points()).sum()
}

fn describe(s: &Settings, by_tpn: &HashMap<u32, &Player>, p: &Player, done: usize) -> Info {
    let score = score_before(p, done);
    let mut played = Vec::new();
    let mut opponents = Vec::new();
    let mut bye_ok = true;
    let mut unplayed = 0;
    for r in &p.history {
        match *r {
            Round::Played {
                opponent, colour, ..
            } => {
                played.push(colour);
                opponents.push(opponent);
            }
            Round::PairingBye { .. } => {
                bye_ok = false;
                unplayed += 1;
            }
            Round::Forfeit { points, .. } | Round::Absent { points } => {
                if points >= s.win {
                    bye_ok = false;
                }
                unplayed += 1;
            }
        }
    }
    let float_in = |round: usize| -> Float {
        match p.history[round] {
            Round::Played { opponent, .. } => {
                let mine = score_before(p, round);
                let theirs = by_tpn
                    .get(&opponent)
                    .map_or(mine, |o| score_before(o, round));
                match mine.cmp(&theirs) {
                    std::cmp::Ordering::Greater => Float::Down,
                    std::cmp::Ordering::Less => Float::Up,
                    std::cmp::Ordering::Equal => Float::None,
                }
            }
            Round::PairingBye { .. } => Float::Down,
            Round::Forfeit { points, .. } | Round::Absent { points } => {
                if points > s.loss {
                    Float::Down
                } else {
                    Float::None
                }
            }
        }
    };
    let float = [
        if done >= 1 {
            float_in(done - 1)
        } else {
            Float::None
        },
        if done >= 2 {
            float_in(done - 2)
        } else {
            Float::None
        },
    ];
    // 1.8: over 50% of the maximum possible score, when pairing the final round.
    let topscorer = done as u32 + 1 == s.rounds && 2 * score > s.win * done as u32;
    let pref = colour::preference(&played);
    Info {
        tpn: p.tpn,
        score,
        played,
        opponents,
        bye_ok,
        float,
        unplayed,
        topscorer,
        pref,
    }
}

/// Distinct scores, lowest first: a score's index is its C5 level.
fn score_levels(info: &[Info]) -> Vec<u32> {
    let mut v: Vec<u32> = info.iter().map(|i| i.score).collect();
    v.sort_unstable();
    v.dedup();
    v
}

struct Ctx<'a> {
    s: &'a Settings,
    info: &'a [Info],
    levels: Vec<u32>,
}

impl Ctx<'_> {
    fn pair(&self) -> Result<Pairing, Error> {
        let n = self.info.len();
        let mut brackets = Vec::new();
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        let mut mdps: Vec<usize> = Vec::new();
        let mut pos = 0;
        while pos < n {
            let score = self.info[pos].score;
            let end = (pos..n).find(|&i| self.info[i].score != score).unwrap_or(n);
            let mut b: Vec<usize> = mdps.clone();
            b.extend(pos..end);
            let lower: Vec<usize> = (end..n).collect();
            let solved = BracketSolver::new(self, &b, mdps.len(), &lower)?.solve()?;
            pairs.extend(solved.pairs.iter().copied());
            brackets.push(self.report(score, &b, mdps.len(), &solved));
            mdps = solved.floaters;
            pos = end;
        }
        if mdps.len() > 1 {
            return Err(Error::NoLegalPairing);
        }
        let mut boards: Vec<(usize, usize)> = pairs;
        // C.04.2 3.6: higher ranked player's score, then the sum, then that player's TPN.
        boards.sort_by(|&(a, b), &(c, d)| {
            let (i, j) = (&self.info, &self.info);
            i[c].score
                .cmp(&i[a].score)
                .then((j[c].score + j[d].score).cmp(&(j[a].score + j[b].score)))
                .then(i[a].tpn.cmp(&i[c].tpn))
        });
        let boards = boards
            .into_iter()
            .map(|(hi, lo)| self.board(hi, lo))
            .collect();
        Ok(Pairing {
            boards,
            bye: mdps.first().map(|&i| self.info[i].tpn),
            brackets,
        })
    }

    /// Colours for a pair, `hi` ranked above `lo`.
    fn colour_of_higher(&self, hi: usize, lo: usize) -> Colour {
        let (a, b) = (&self.info[hi], &self.info[lo]);
        colour::allocate(
            &Side {
                played: &a.played,
                tpn: a.tpn,
            },
            &Side {
                played: &b.played,
                tpn: b.tpn,
            },
            self.s.initial_colour,
        )
    }

    fn board(&self, hi: usize, lo: usize) -> Board {
        let (a, b) = (self.info[hi].tpn, self.info[lo].tpn);
        if self.colour_of_higher(hi, lo) == Colour::White {
            Board { white: a, black: b }
        } else {
            Board { white: b, black: a }
        }
    }

    /// C1 and C3.
    fn compatible(&self, a: usize, b: usize) -> bool {
        let (x, y) = (&self.info[a], &self.info[b]);
        if x.opponents.contains(&y.tpn) {
            return false;
        }
        let both_absolute_same = matches!((x.pref, y.pref), (Some(p), Some(q))
            if p.strength == Strength::Absolute && q.strength == Strength::Absolute && p.colour == q.colour);
        !(both_absolute_same && !x.topscorer && !y.topscorer)
    }

    fn report(&self, score: u32, b: &[usize], m0: usize, solved: &Solved) -> Bracket {
        let tpn = |i: usize| self.info[i].tpn;
        Bracket {
            score,
            residents: b[m0..].iter().map(|&i| tpn(i)).collect(),
            moved_down: b[..m0].iter().map(|&i| tpn(i)).collect(),
            pairs: solved
                .pairs
                .iter()
                .map(|&(x, y)| (tpn(x), tpn(y)))
                .collect(),
            floaters: solved.floaters.iter().map(|&i| tpn(i)).collect(),
            natural: solved.natural,
            exchange: solved.exchange.as_ref().map(|(x, y)| {
                (
                    x.iter().map(|&i| tpn(i)).collect(),
                    y.iter().map(|&i| tpn(i)).collect(),
                )
            }),
            failures: solved.failures.clone(),
        }
    }
}

/// The outcome of one bracket, in round indices.
struct Solved {
    pairs: Vec<(usize, usize)>,
    floaters: Vec<usize>,
    natural: bool,
    exchange: Option<(Vec<usize>, Vec<usize>)>,
    failures: Vec<(Criterion, u32)>,
}

/// Bits for criterion fields, highest priority first.
struct Layout {
    widths: Vec<u32>,
    shifts: Vec<u32>,
}

impl Layout {
    fn add(&mut self, max: u64) -> usize {
        self.widths.push(64 - max.leading_zeros() + 1);
        self.widths.len() - 1
    }

    fn finish(&mut self) -> u32 {
        let total: u32 = self.widths.iter().sum();
        let mut at = total;
        self.shifts = self
            .widths
            .iter()
            .map(|w| {
                at -= w;
                at
            })
            .collect();
        total
    }
}

/// Vertex kinds in a bracket's matching graph.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Mdp,
    Resident,
    /// The next scoregroup.
    Next,
    /// Every lower scoregroup.
    Below,
    Bye,
}

/// Field ids of one bracket's layout.
struct Fields {
    c5: usize,
    c6: usize,
    c7: Vec<(u32, usize)>,
    c8a: usize,
    c8b: Vec<(u32, usize)>,
    c9: usize,
    c10_17: [usize; 8],
    c18: Vec<(u32, usize)>,
    c19: Vec<(u32, usize)>,
    c20: Vec<(u32, usize)>,
    c21: Vec<(u32, usize)>,
    tie: usize,
}

/// Score difference used for a moved-down player who floats again: above any real one.
const FLOATS_AGAIN: u32 = u32::MAX;

fn level(list: &[(u32, usize)], key: u32) -> usize {
    list.iter()
        .find(|(k, _)| *k == key)
        .map(|&(_, f)| f)
        .expect("level field")
}

/// Distinct keys, highest first, each with a field of width for `max`.
fn levels(layout: &mut Layout, mut keys: Vec<u32>, max: u64) -> Vec<(u32, usize)> {
    keys.sort_unstable_by(|a, b| b.cmp(a));
    keys.dedup();
    keys.into_iter().map(|k| (k, layout.add(max))).collect()
}

struct BracketSolver<'r, 'a> {
    r: &'r Ctx<'a>,
    /// Bracket players (round indices), MDPs first, in bracket order: index = BSN - 1.
    b: Vec<usize>,
    m0: usize,
    /// Vertex -> round index (`usize::MAX` for the bye vertex).
    verts: Vec<usize>,
    kinds: Vec<Kind>,
    edges: Vec<(usize, usize, Wide)>,
    /// Each edge's reduced cost under the best matching's duals, once known.
    rc: Vec<Wide>,
    offset: Wide,
    layout: Layout,
    fields: Fields,
}

/// What a choice so far fixes in a bracket.
#[derive(Clone)]
struct Constraint {
    forced: Vec<(usize, usize)>,
    /// Bracket vertices that must be paired inside the bracket (S1, S1R).
    must_pair: Vec<bool>,
    /// Bracket vertices that may not be paired inside the bracket (the Limbo).
    must_float: Vec<bool>,
    /// When set, remainder pairs must join an S1R vertex (true) and an S2R vertex (false).
    side: Option<Vec<Option<bool>>>,
}

impl Constraint {
    fn allows(&self, nb: usize, u: usize, v: usize) -> bool {
        let (ub, vb) = (u < nb, v < nb);
        if ub && vb {
            if self.must_float[u] || self.must_float[v] {
                return false;
            }
            if let Some(side) = &self.side
                && let (Some(su), Some(sv)) = (side[u], side[v])
            {
                return su != sv;
            }
            true
        } else if ub {
            !self.must_pair[u]
        } else if vb {
            !self.must_pair[v]
        } else {
            true
        }
    }
}

impl<'r, 'a> BracketSolver<'r, 'a> {
    fn new(r: &'r Ctx<'a>, b: &[usize], m0: usize, lower: &[usize]) -> Result<Self, Error> {
        let info = r.info;
        let nb = b.len();
        let next_score = lower.first().map(|&i| info[i].score);
        let mut verts: Vec<usize> = b.to_vec();
        let mut kinds: Vec<Kind> = (0..nb)
            .map(|i| if i < m0 { Kind::Mdp } else { Kind::Resident })
            .collect();
        for &i in lower {
            verts.push(i);
            kinds.push(if Some(info[i].score) == next_score {
                Kind::Next
            } else {
                Kind::Below
            });
        }
        if verts.len() % 2 == 1 {
            verts.push(usize::MAX);
            kinds.push(Kind::Bye);
        }
        let n_next = kinds.iter().filter(|k| **k == Kind::Next).count();
        let resident_score = info[b[m0.min(nb - 1)]].score;
        let mdp_diffs: Vec<u32> = b[..m0]
            .iter()
            .map(|&i| info[i].score - resident_score)
            .collect();
        let had = |i: usize, back: usize, f: Float| info[i].float[back] == f;
        let nbu = nb as u64;

        let mut layout = Layout {
            widths: Vec::new(),
            shifts: Vec::new(),
        };
        let c5 = layout.add(r.levels.len() as u64);
        let c6 = layout.add(nbu);
        let c7 = levels(&mut layout, b.iter().map(|&i| info[i].score).collect(), nbu);
        let c8a = layout.add(nbu + n_next as u64);
        let mut c8_keys: Vec<u32> = b.iter().map(|&i| info[i].score).collect();
        c8_keys.extend(next_score);
        let c8b = levels(&mut layout, c8_keys, nbu + n_next as u64);
        let c9 = layout.add(r.s.rounds as u64);
        let c10_17 = [(); 8].map(|_| layout.add(2 * nbu));
        let mdp_keys = |back: usize, also_again: bool| -> Vec<u32> {
            let mut k: Vec<u32> = (0..m0)
                .filter(|&x| had(b[x], back, Float::Down))
                .map(|x| mdp_diffs[x])
                .collect();
            if also_again && !k.is_empty() {
                k.push(FLOATS_AGAIN);
            }
            k
        };
        let opp_keys = |back: usize| -> Vec<u32> {
            if b[m0..].iter().any(|&i| had(i, back, Float::Up)) {
                mdp_diffs.clone()
            } else {
                Vec::new()
            }
        };
        let c18 = levels(&mut layout, mdp_keys(0, true), nbu);
        let c19 = levels(&mut layout, opp_keys(0), nbu);
        let c20 = levels(&mut layout, mdp_keys(1, true), nbu);
        let c21 = levels(&mut layout, opp_keys(1), nbu);
        let tie = layout.add(nbu * nbu + nbu);
        let total = layout.finish();
        if total + 4 >= BITS {
            return Err(Error::TooComplex);
        }
        let fields = Fields {
            c5,
            c6,
            c7,
            c8a,
            c8b,
            c9,
            c10_17,
            c18,
            c19,
            c20,
            c21,
            tie,
        };

        let mut s = BracketSolver {
            r,
            b: b.to_vec(),
            m0,
            verts,
            kinds,
            edges: Vec::new(),
            rc: Vec::new(),
            offset: Wide::shifted(1, total),
            layout,
            fields,
        };
        s.build_edges(&mdp_diffs);
        Ok(s)
    }

    fn encode(&self, cost: &[(usize, u64)]) -> Wide {
        let mut w = self.offset;
        for &(f, v) in cost {
            w = w - Wide::shifted(v, self.layout.shifts[f]);
        }
        w
    }

    /// Costs of a bracket player `x` (vertex index) leaving the bracket.
    fn float_cost(&self, x: usize, cost: &mut Vec<(usize, u64)>) {
        let info = &self.r.info[self.verts[x]];
        let f = &self.fields;
        cost.push((f.c6, 1));
        cost.push((level(&f.c7, info.score), 1));
        if self.kinds[x] == Kind::Resident {
            if info.float[0] == Float::Down {
                cost.push((f.c10_17[4], 1));
            }
            if info.float[1] == Float::Down {
                cost.push((f.c10_17[6], 1));
            }
        } else {
            if info.float[0] == Float::Down {
                cost.push((level(&f.c18, FLOATS_AGAIN), 1));
            }
            if info.float[1] == Float::Down {
                cost.push((level(&f.c20, FLOATS_AGAIN), 1));
            }
        }
        // Tie-break heuristic only: S1 players should not float.
        if x < self.m0 || x - self.m0 < (self.b.len() - self.m0) / 2 {
            cost.push((f.tie, self.b.len() as u64));
        }
    }

    /// Costs of `x` (bracket or next-scoregroup vertex) being a downfloater of the next bracket.
    fn next_float_cost(&self, x: usize, cost: &mut Vec<(usize, u64)>) {
        let info = &self.r.info[self.verts[x]];
        cost.push((self.fields.c8a, 1));
        cost.push((level(&self.fields.c8b, info.score), 1));
    }

    fn build_edges(&mut self, mdp_diffs: &[u32]) {
        let nb = self.b.len();
        let nv = self.verts.len();
        let info = self.r.info;
        let f = &self.fields;
        let mut edges = Vec::new();
        let mut cost: Vec<(usize, u64)> = Vec::new();
        let has_next = self.kinds.contains(&Kind::Next);
        for u in 0..nv {
            for v in u + 1..nv {
                cost.clear();
                let (ku, kv) = (self.kinds[u], self.kinds[v]);
                if kv == Kind::Bye {
                    let p = &info[self.verts[u]];
                    if !p.bye_ok {
                        continue;
                    }
                    let lvl = self.r.levels.iter().position(|&s| s == p.score).unwrap() as u64;
                    cost.push((f.c5, lvl));
                    match ku {
                        Kind::Mdp | Kind::Resident => {
                            self.float_cost(u, &mut cost);
                            if has_next {
                                self.next_float_cost(u, &mut cost);
                            }
                            cost.push((f.c9, p.unplayed as u64));
                        }
                        Kind::Next => self.next_float_cost(u, &mut cost),
                        _ => {}
                    }
                    edges.push((u, v, self.encode(&cost)));
                    continue;
                }
                let (a, c) = (self.verts[u], self.verts[v]);
                if !self.r.compatible(a, c) {
                    continue;
                }
                let (in_u, in_v) = (u < nb, v < nb);
                if in_u && in_v {
                    if ku == Kind::Mdp && kv == Kind::Mdp {
                        continue;
                    }
                    self.pair_cost(u, v, mdp_diffs, &mut cost);
                } else if in_u {
                    self.float_cost(u, &mut cost);
                    if kv == Kind::Below {
                        self.next_float_cost(u, &mut cost);
                    }
                } else if ku != kv {
                    // One in the next scoregroup, one below it.
                    let x = if ku == Kind::Next { u } else { v };
                    self.next_float_cost(x, &mut cost);
                }
                edges.push((u, v, self.encode(&cost)));
            }
        }
        self.edges = edges;
    }

    /// Costs of the pair `u`-`v` inside the bracket (`u` ranked above `v`).
    fn pair_cost(&self, u: usize, v: usize, mdp_diffs: &[u32], cost: &mut Vec<(usize, u64)>) {
        let info = self.r.info;
        let f = &self.fields;
        let (hi, lo) = (self.verts[u], self.verts[v]);
        let col_hi = self.r.colour_of_higher(hi, lo);
        let top = info[hi].topscorer || info[lo].topscorer;
        for (p, got) in [(hi, col_hi), (lo, col_hi.opposite())] {
            let i = &info[p];
            if let Some(pref) = i.pref
                && pref.colour != got
            {
                cost.push((f.c10_17[2], 1));
                if pref.strength >= Strength::Strong {
                    cost.push((f.c10_17[3], 1));
                }
            }
            if top {
                let cd = colour::difference(&i.played) + if got == Colour::White { 1 } else { -1 };
                if cd.abs() > 2 {
                    cost.push((f.c10_17[0], 1));
                }
                let n = i.played.len();
                if n >= 2 && i.played[n - 1] == got && i.played[n - 2] == got {
                    cost.push((f.c10_17[1], 1));
                }
            }
        }
        let nb = self.b.len() as u64;
        if self.kinds[u] == Kind::Mdp {
            let (m, r) = (&info[hi], &info[lo]);
            let diff = mdp_diffs[u];
            if r.float[0] == Float::Up {
                cost.push((f.c10_17[5], 1));
                cost.push((level(&f.c19, diff), 1));
            }
            if r.float[1] == Float::Up {
                cost.push((f.c10_17[7], 1));
                cost.push((level(&f.c21, diff), 1));
            }
            if m.float[0] == Float::Down {
                cost.push((level(&f.c18, diff), 1));
            }
            if m.float[1] == Float::Down {
                cost.push((level(&f.c20, diff), 1));
            }
            let (bu, bv) = (u as u64, v as u64 - self.m0 as u64);
            cost.push((f.tie, bu.abs_diff(bv)));
        } else {
            let half = (nb - self.m0 as u64) / 2;
            let (bu, bv) = (u as u64 - self.m0 as u64, v as u64 - self.m0 as u64);
            cost.push((f.tie, (bv - bu).abs_diff(half)));
        }
    }

    /// The best value under a constraint, with a matching that reaches it.
    fn best(&self, c: &Constraint) -> Option<(Wide, Vec<Option<usize>>)> {
        self.best_on(c, None)
    }

    /// [`best`](Self::best) over the vertices in `keep` only; the others come
    /// in pairs whose edges weigh exactly the offset.
    fn best_on(&self, c: &Constraint, keep: Option<&[bool]>) -> Option<(Wide, Vec<Option<usize>>)> {
        let nb = self.b.len();
        let nv = self.verts.len();
        let kept = |u: usize| keep.is_none_or(|k| k[u]);
        let mut at = vec![usize::MAX; nv];
        let mut back = Vec::with_capacity(nv);
        for u in (0..nv).filter(|&u| kept(u)) {
            at[u] = back.len();
            back.push(u);
        }
        let mut fixed = vec![false; nv];
        let mut total = Wide::ZERO;
        for &(x, y) in &c.forced {
            fixed[x] = true;
            fixed[y] = true;
            let w = self
                .edges
                .iter()
                .find(|e| (e.0 == x && e.1 == y) || (e.0 == y && e.1 == x))?
                .2;
            total = total + w;
        }
        let edges: Vec<(usize, usize, Wide)> = self
            .edges
            .iter()
            .filter(|&&(u, v, _)| {
                kept(u) && kept(v) && !fixed[u] && !fixed[v] && c.allows(nb, u, v)
            })
            .map(|&(u, v, w)| (at[u], at[v], w))
            .collect();
        let sub = wide_matching(back.len(), &edges);
        let mut mate = vec![None; nv];
        for (i, m) in sub.iter().enumerate() {
            mate[back[i]] = m.map(|j| back[j]);
        }
        if sub
            .iter()
            .zip(&back)
            .any(|(m, &u)| m.is_none() && !fixed[u])
        {
            return None;
        }
        for &(x, y) in &c.forced {
            mate[x] = Some(y);
            mate[y] = Some(x);
        }
        for &(u, v, w) in &edges {
            if sub[u] == Some(v) {
                total = total + w;
            }
        }
        Some((total, mate))
    }

    /// [`best`](Self::best), trying first a smaller graph: the witness's pairs
    /// of two next-scoregroup or two lower players, all of equal weight, are
    /// left out but for a few of each kind. What the smaller graph reaches, the
    /// whole graph reaches with those pairs added back.
    fn best_near(
        &self,
        c: &Constraint,
        witness: &[Option<usize>],
        target: Wide,
    ) -> Option<(Wide, Vec<Option<usize>>)> {
        const RESERVE: usize = 4;
        let nb = self.b.len();
        let nv = self.verts.len();
        let mut keep = vec![true; nv];
        let mut reserve = [0usize; 2];
        let mut dropped = Vec::new();
        for u in nb..nv {
            let Some(v) = witness[u] else { continue };
            if v <= u || self.kinds[u] != self.kinds[v] {
                continue;
            }
            let k = match self.kinds[u] {
                Kind::Next => 0,
                Kind::Below => 1,
                _ => continue,
            };
            if reserve[k] < RESERVE {
                reserve[k] += 1;
            } else {
                keep[u] = false;
                keep[v] = false;
                dropped.push((u, v));
            }
        }
        if !dropped.is_empty()
            && let Some((mut value, mut mate)) = self.best_on(c, Some(&keep))
        {
            for &(u, v) in &dropped {
                mate[u] = Some(v);
                mate[v] = Some(u);
                value = value + self.offset;
            }
            if self.agrees(value, target) {
                return Some((value, mate));
            }
        }
        self.best(c)
    }

    fn satisfies(&self, mate: &[Option<usize>], c: &Constraint) -> bool {
        let nb = self.b.len();
        c.forced.iter().all(|&(x, y)| mate[x] == Some(y))
            && (0..mate.len()).all(|u| match mate[u] {
                Some(v) if u < v => {
                    c.forced
                        .iter()
                        .any(|&(x, y)| (x == u && y == v) || (x == v && y == u))
                        || c.allows(nb, u, v)
                }
                _ => true,
            })
    }

    fn solve(&self) -> Result<Solved, Error> {
        let nb = self.b.len();
        let open = Constraint {
            forced: Vec::new(),
            must_pair: vec![false; nb],
            must_float: vec![false; nb],
            side: None,
        };
        let (target, witness, rc) = self.open_best().ok_or(Error::NoLegalPairing)?;
        // C9 applies only to a bracket whose single downfloater takes the bye.
        let floats = (0..nb)
            .filter(|&x| witness[x].is_some_and(|y| y >= nb))
            .count();
        if floats != 1 {
            let other = self.without_c9();
            let (target, witness, rc) = other.open_best().ok_or(Error::NoLegalPairing)?;
            return other.near_best(&rc).walk(target, witness, open);
        }
        self.near_best(&rc).walk(target, witness, open)
    }

    /// The best matching with no constraint, and each edge's reduced cost.
    fn open_best(&self) -> Option<(Wide, Vec<Option<usize>>, Vec<Wide>)> {
        let (mate, rc) = wide_matching_duals(self.verts.len(), &self.edges);
        if mate.iter().any(|m| m.is_none()) {
            return None;
        }
        let total = self
            .edges
            .iter()
            .filter(|&&(u, v, _)| mate[u] == Some(v))
            .fold(Wide::ZERO, |t, e| t + e.2);
        Some((total, mate, rc))
    }

    /// The same bracket without the edges no candidate at the best values can use.
    ///
    /// A matching that agrees with the best on every criterion differs from it
    /// in the tie-break field only, the lowest, so it weighs less by under
    /// `2^width`. An edge whose reduced cost (in quarter weights) exceeds that
    /// is in no such matching.
    fn near_best(&self, rc: &[Wide]) -> BracketSolver<'r, 'a> {
        let limit = Wide::shifted(4, self.layout.widths[self.fields.tie]);
        let mut s = self.without_edges();
        (s.edges, s.rc) = self
            .edges
            .iter()
            .zip(rc)
            .filter(|&(_, &r)| r < limit)
            .unzip();
        s
    }

    /// The same bracket with C9 switched off.
    fn without_c9(&self) -> BracketSolver<'r, 'a> {
        let shift = self.layout.shifts[self.fields.c9];
        let width = self.layout.widths[self.fields.c9];
        let mut s = self.without_edges();
        s.fields.c9 = usize::MAX;
        s.edges = self
            .edges
            .iter()
            .map(|&(u, v, w)| {
                let c9 = (self.offset - w).field(shift, width);
                (u, v, w + Wide::shifted(c9, shift))
            })
            .collect();
        s
    }

    /// A copy with no edges.
    fn without_edges(&self) -> BracketSolver<'r, 'a> {
        BracketSolver {
            r: self.r,
            b: self.b.clone(),
            m0: self.m0,
            verts: self.verts.clone(),
            kinds: self.kinds.clone(),
            edges: Vec::new(),
            rc: Vec::new(),
            offset: self.offset,
            layout: Layout {
                widths: self.layout.widths.clone(),
                shifts: self.layout.shifts.clone(),
            },
            fields: Fields {
                c7: self.fields.c7.clone(),
                c8b: self.fields.c8b.clone(),
                c18: self.fields.c18.clone(),
                c19: self.fields.c19.clone(),
                c20: self.fields.c20.clone(),
                c21: self.fields.c21.clone(),
                ..self.fields
            },
        }
    }

    /// Try a constraint: feasible at the target value? Keeps the witness up to date.
    fn reaches(&self, target: Wide, c: &Constraint, witness: &mut Vec<Option<usize>>) -> bool {
        if self.satisfies(witness, c) {
            return true;
        }
        if self.out_of_reach(c) {
            return false;
        }
        match self.best_near(c, witness, target) {
            Some((v, m)) if self.agrees(v, target) => {
                *witness = m;
                true
            }
            _ => false,
        }
    }

    /// True when the duals prove that no matching under `c` agrees with the
    /// best: each vertex needs an edge, and its cheapest allowed one already
    /// costs more than the tie-break field can make up.
    fn out_of_reach(&self, c: &Constraint) -> bool {
        if self.rc.is_empty() {
            return false;
        }
        let nb = self.b.len();
        let nv = self.verts.len();
        let mut fixed = vec![false; nv];
        let mut loss = Wide::ZERO;
        for &(x, y) in &c.forced {
            fixed[x] = true;
            fixed[y] = true;
            let Some(k) = self
                .edges
                .iter()
                .position(|e| (e.0 == x && e.1 == y) || (e.0 == y && e.1 == x))
            else {
                return true;
            };
            loss = loss + self.rc[k] + self.rc[k];
        }
        let mut cheapest: Vec<Option<Wide>> = vec![None; nv];
        for (&(u, v, _), &r) in self.edges.iter().zip(&self.rc) {
            if fixed[u] || fixed[v] || !c.allows(nb, u, v) {
                continue;
            }
            for x in [u, v] {
                if cheapest[x].is_none_or(|m| r < m) {
                    cheapest[x] = Some(r);
                }
            }
        }
        for x in (0..nv).filter(|&x| !fixed[x]) {
            match cheapest[x] {
                Some(r) => loss = loss + r,
                None => return true,
            }
        }
        // `loss` counts every edge twice, in quarter weights.
        loss >= Wide::shifted(8, self.layout.widths[self.fields.tie])
    }

    /// Equal on every criterion field; the tie-break heuristic does not count.
    fn agrees(&self, a: Wide, b: Wide) -> bool {
        let (ca, cb) = (self.offset_cost(a), self.offset_cost(b));
        (0..self.layout.widths.len())
            .filter(|&f| f != self.fields.tie)
            .all(|f| {
                ca.field(self.layout.shifts[f], self.layout.widths[f])
                    == cb.field(self.layout.shifts[f], self.layout.widths[f])
            })
    }

    /// Total cost of a full matching's value.
    fn offset_cost(&self, value: Wide) -> Wide {
        let pairs = self.verts.len() / 2;
        let mut all = Wide::ZERO;
        for _ in 0..pairs {
            all = all + self.offset;
        }
        all - value
    }

    fn walk(
        &self,
        target: Wide,
        mut witness: Vec<Option<usize>>,
        mut c: Constraint,
    ) -> Result<Solved, Error> {
        let nb = self.b.len();
        let m0 = self.m0;
        let info = self.r.info;
        let floats_in = |w: &[Option<usize>], x: usize| w[x].is_some_and(|y| y >= nb);
        // The Limbo's size and scores are fixed by C6 and C7.
        let limbo: Vec<usize> = (0..m0).filter(|&x| floats_in(&witness, x)).collect();
        let resident_floaters = (m0..nb).filter(|&x| floats_in(&witness, x)).count();
        let mut limbo_scores: Vec<u32> = limbo.iter().map(|&x| info[self.b[x]].score).collect();
        limbo_scores.sort_unstable();
        let m1 = m0 - limbo.len();
        let mut natural = true;

        // 4.4: the set of MDPs to pair, smallest differing BSN first.
        if m0 > 0 {
            let mut chosen = None;
            for set in combinations(m0, m1) {
                let mut out: Vec<u32> = (0..m0)
                    .filter(|x| !set.contains(x))
                    .map(|x| info[self.b[x]].score)
                    .collect();
                out.sort_unstable();
                if out != limbo_scores {
                    continue;
                }
                let mut t = c.clone();
                for x in 0..m0 {
                    t.must_pair[x] = set.contains(&x);
                    t.must_float[x] = !set.contains(&x);
                }
                if self.reaches(target, &t, &mut witness) {
                    chosen = Some(set);
                    c = t;
                    break;
                }
            }
            let set = chosen.ok_or(Error::NoLegalPairing)?;
            if set != (0..m1).collect::<Vec<_>>() {
                natural = false;
            }
            // 4.2 on the MDP-pairing: each MDP takes the first resident that still works.
            let mut used = vec![false; nb];
            for (k, &m) in set.iter().enumerate() {
                let mut done = false;
                for r in (m0..nb).filter(|&r| !used[r]) {
                    let mut t = c.clone();
                    t.forced.push((m, r));
                    if self.reaches(target, &t, &mut witness) {
                        if r != m0 + k {
                            natural = false;
                        }
                        used[r] = true;
                        c = t;
                        done = true;
                        break;
                    }
                }
                if !done {
                    return Err(Error::NoLegalPairing);
                }
            }
        }

        // The remainder (or the whole homogeneous bracket): residents not paired with an MDP.
        let rem: Vec<usize> = (m0..nb)
            .filter(|&x| !c.forced.iter().any(|&(_, r)| r == x))
            .collect();
        let p = (rem.len() - resident_floaters) / 2;
        let s1: Vec<usize> = rem[..p].to_vec();
        let s2: Vec<usize> = rem[p..].to_vec();
        let mut exchange = None;
        let mut split = None;
        'ex: for size in 0..=s1.len().min(s2.len()) {
            for (x, y) in exchanges(s1.len(), s2.len(), size) {
                let new_s1: Vec<usize> = {
                    let mut v: Vec<usize> = (0..s1.len())
                        .filter(|i| !x.contains(i))
                        .map(|i| s1[i])
                        .collect();
                    v.extend(y.iter().map(|&j| s2[j]));
                    v.sort_unstable();
                    v
                };
                let new_s2: Vec<usize> = {
                    let mut v: Vec<usize> = (0..s2.len())
                        .filter(|j| !y.contains(j))
                        .map(|j| s2[j])
                        .collect();
                    v.extend(x.iter().map(|&i| s1[i]));
                    v.sort_unstable();
                    v
                };
                let mut t = c.clone();
                let mut side = vec![None; nb];
                for &a in &new_s1 {
                    side[a] = Some(true);
                    t.must_pair[a] = true;
                }
                for &a in &new_s2 {
                    side[a] = Some(false);
                }
                t.side = Some(side);
                if self.reaches(target, &t, &mut witness) {
                    if size > 0 {
                        natural = false;
                        exchange = Some((
                            x.iter().map(|&i| self.b[s1[i]]).collect(),
                            y.iter().map(|&j| self.b[s2[j]]).collect(),
                        ));
                    }
                    c = t;
                    split = Some((new_s1, new_s2));
                    break 'ex;
                }
            }
        }
        let (s1, s2) = split.ok_or(Error::NoLegalPairing)?;
        // 4.2 in the remainder: each S1 player takes the first S2 player that still works.
        let mut used = vec![false; nb];
        for (k, &a) in s1.iter().enumerate() {
            let mut done = false;
            for (j, &bv) in s2.iter().enumerate() {
                if used[bv] {
                    continue;
                }
                let mut t = c.clone();
                t.forced.push((a, bv));
                if self.reaches(target, &t, &mut witness) {
                    if j != k {
                        natural = false;
                    }
                    used[bv] = true;
                    c = t;
                    done = true;
                    break;
                }
            }
            if !done {
                return Err(Error::NoLegalPairing);
            }
        }

        let pairs = c
            .forced
            .iter()
            .map(|&(x, y)| (self.b[x.min(y)], self.b[x.max(y)]))
            .collect();
        let floaters = (0..nb)
            .filter(|&x| floats_in(&witness, x))
            .map(|x| self.b[x])
            .collect();
        Ok(Solved {
            pairs,
            floaters,
            natural,
            exchange,
            failures: self.failures(target),
        })
    }

    /// Scores a given choice for this bracket against the rules' best.
    fn judge(&self, theirs: &[(usize, usize)], given: &[Option<usize>], score: u32) -> Verdict {
        let nb = self.b.len();
        let vertex = |x: usize| self.b.iter().position(|&y| y == x).unwrap();
        let mut c = Constraint {
            forced: Vec::new(),
            must_pair: vec![false; nb],
            must_float: vec![false; nb],
            side: None,
        };
        for &(x, y) in theirs {
            let (u, v) = (vertex(x), vertex(y));
            if !self
                .edges
                .iter()
                .any(|e| (e.0 == u && e.1 == v) || (e.0 == v && e.1 == u))
            {
                return Verdict::Illegal { bracket: score };
            }
            c.forced.push((u.min(v), u.max(v)));
        }
        for (u, &x) in self.b.iter().enumerate() {
            if !given[x].is_some_and(|y| self.b.contains(&y)) {
                c.must_float[u] = true;
            }
        }
        let open = Constraint {
            forced: Vec::new(),
            must_pair: vec![false; nb],
            must_float: vec![false; nb],
            side: None,
        };
        let Some((best, _)) = self.best(&open) else {
            return Verdict::Illegal { bracket: score };
        };
        let Some((value, _)) = self.best(&c) else {
            return Verdict::Illegal { bracket: score };
        };
        let (cb, cv) = (self.offset_cost(best), self.offset_cost(value));
        for f in 0..self.layout.widths.len() {
            if f == self.fields.tie {
                continue;
            }
            let (a, b) = (
                cb.field(self.layout.shifts[f], self.layout.widths[f]),
                cv.field(self.layout.shifts[f], self.layout.widths[f]),
            );
            if a != b {
                let criterion = self.criterion_of(f);
                return if b > a {
                    Verdict::Worse {
                        bracket: score,
                        criterion,
                    }
                } else {
                    Verdict::Better {
                        bracket: score,
                        criterion,
                    }
                };
            }
        }
        Verdict::GeneratedLater { bracket: score }
    }

    fn criterion_of(&self, f: usize) -> Criterion {
        use Criterion::*;
        let fl = &self.fields;
        let within = |l: &[(u32, usize)]| l.iter().any(|&(_, x)| x == f);
        if f == fl.c5 {
            ByeScore
        } else if f == fl.c6 {
            Downfloaters
        } else if within(&fl.c7) {
            DownfloaterScores
        } else if f == fl.c8a {
            NextBracketDownfloaters
        } else if within(&fl.c8b) {
            NextBracketDownfloaterScores
        } else if f == fl.c9 {
            ByeUnplayedRounds
        } else if let Some(k) = fl.c10_17.iter().position(|&x| x == f) {
            [
                TopscorerColourDifference,
                TopscorerColourRepeat,
                ColourPreference,
                StrongColourPreference,
                RepeatDownfloat,
                RepeatUpfloat,
                RepeatDownfloatTwoRounds,
                RepeatUpfloatTwoRounds,
            ][k]
        } else if within(&fl.c18) {
            RepeatMovedDown
        } else if within(&fl.c19) {
            RepeatMovedDownOpponent
        } else if within(&fl.c20) {
            RepeatMovedDownTwoRounds
        } else {
            RepeatMovedDownOpponentTwoRounds
        }
    }

    fn failures(&self, value: Wide) -> Vec<(Criterion, u32)> {
        let cost = self.offset_cost(value);
        let get = |f: usize| cost.field(self.layout.shifts[f], self.layout.widths[f]) as u32;
        let sum = |l: &[(u32, usize)]| l.iter().map(|&(_, f)| get(f)).sum::<u32>();
        let f = &self.fields;
        use Criterion::*;
        let mut out = vec![
            (Downfloaters, get(f.c6)),
            (NextBracketDownfloaters, get(f.c8a)),
            (
                ByeUnplayedRounds,
                if f.c9 == usize::MAX { 0 } else { get(f.c9) },
            ),
            (TopscorerColourDifference, get(f.c10_17[0])),
            (TopscorerColourRepeat, get(f.c10_17[1])),
            (ColourPreference, get(f.c10_17[2])),
            (StrongColourPreference, get(f.c10_17[3])),
            (RepeatDownfloat, get(f.c10_17[4])),
            (RepeatUpfloat, get(f.c10_17[5])),
            (RepeatDownfloatTwoRounds, get(f.c10_17[6])),
            (RepeatUpfloatTwoRounds, get(f.c10_17[7])),
            (RepeatMovedDown, sum(&f.c18)),
            (RepeatMovedDownOpponent, sum(&f.c19)),
            (RepeatMovedDownTwoRounds, sum(&f.c20)),
            (RepeatMovedDownOpponentTwoRounds, sum(&f.c21)),
        ];
        out.retain(|&(_, n)| n > 0);
        out
    }
}

/// k-subsets of 0..n in lexicographic order.
fn combinations(n: usize, k: usize) -> Vec<Vec<usize>> {
    fn go(start: usize, n: usize, k: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..n {
            if n - i < k - cur.len() {
                break;
            }
            cur.push(i);
            go(i + 1, n, k, cur, out);
            cur.pop();
        }
    }
    let mut out = Vec::new();
    go(0, n, k, &mut Vec::new(), &mut out);
    out
}

/// Resident exchanges of `size` players between S1 (positions 0..n1) and S2
/// (positions 0..n2), sorted by 4.3.2. BSNs are positions plus one, S2's after S1's.
fn exchanges(n1: usize, n2: usize, size: usize) -> Vec<(Vec<usize>, Vec<usize>)> {
    if size == 0 {
        return vec![(Vec::new(), Vec::new())];
    }
    let xs = combinations(n1, size);
    let ys = combinations(n2, size);
    let mut all: Vec<(Vec<usize>, Vec<usize>)> = Vec::with_capacity(xs.len() * ys.len());
    for x in &xs {
        for y in &ys {
            all.push((x.clone(), y.clone()));
        }
    }
    let bsn_sum = |x: &[usize], base: usize| x.iter().map(|&i| i + 1 + base).sum::<usize>();
    all.sort_by(|(xa, ya), (xb, yb)| {
        let da = bsn_sum(ya, n1) - bsn_sum(xa, 0);
        let db = bsn_sum(yb, n1) - bsn_sum(xb, 0);
        da.cmp(&db)
            // The largest differing BSN moved from S1: higher is better.
            .then_with(|| xb.iter().rev().cmp(xa.iter().rev()))
            // The smallest differing BSN moved from S2: lower is better.
            .then_with(|| ya.cmp(yb))
    });
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bsn(v: &[(Vec<usize>, Vec<usize>)], n1: usize) -> Vec<(Vec<usize>, Vec<usize>)> {
        v.iter()
            .map(|(x, y)| {
                (
                    x.iter().map(|i| i + 1).collect(),
                    y.iter().map(|j| j + 1 + n1).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn exchange_order_follows_4_3_2() {
        // 11 players: S1 = 1..5, S2 = 6..11.
        let one = bsn(&exchanges(5, 6, 1), 5);
        assert_eq!(one[0], (vec![5], vec![6]));
        let pos =
            |x: Vec<usize>, y: Vec<usize>| one.iter().position(|e| e.0 == x && e.1 == y).unwrap();
        // "exchanging 6 with 4 is better than exchanging 8 with 5"
        assert!(pos(vec![4], vec![6]) < pos(vec![5], vec![8]));
        // Equal difference: the larger BSN from S1 goes first (5-7 before 4-6).
        assert!(pos(vec![5], vec![7]) < pos(vec![4], vec![6]));
        let two = bsn(&exchanges(5, 6, 2), 5);
        let pos2 =
            |x: Vec<usize>, y: Vec<usize>| two.iter().position(|e| e.0 == x && e.1 == y).unwrap();
        // "exchanging 8+6 with 4+3 is better than exchanging 9+8 with 5+4"
        assert!(pos2(vec![3, 4], vec![6, 8]) < pos2(vec![4, 5], vec![8, 9]));
        // "5-2 is better than 4-3" (same sums from S2).
        assert!(pos2(vec![2, 5], vec![6, 7]) < pos2(vec![3, 4], vec![6, 7]));
        // "6-9 is better than 7-8" (same sums from S1).
        assert!(pos2(vec![4, 5], vec![6, 9]) < pos2(vec![4, 5], vec![7, 8]));
        let three = bsn(&exchanges(5, 6, 3), 5);
        let pos3 =
            |x: Vec<usize>, y: Vec<usize>| three.iter().position(|e| e.0 == x && e.1 == y).unwrap();
        // "5-4-1 is better than 5-3-2" and "6-7-10 is better than 6-8-9".
        assert!(pos3(vec![1, 4, 5], vec![6, 7, 8]) < pos3(vec![2, 3, 5], vec![6, 7, 8]));
        assert!(pos3(vec![3, 4, 5], vec![6, 7, 10]) < pos3(vec![3, 4, 5], vec![6, 8, 9]));
    }

    #[test]
    fn mdp_sets_come_in_lexicographic_order() {
        // Annotated Dutch 2026, 4.4.2: {1,3} before {1,4} before {3,4}.
        let sets = combinations(4, 2);
        let p = |s: Vec<usize>| sets.iter().position(|x| *x == s).unwrap();
        assert!(p(vec![0, 2]) < p(vec![0, 3]));
        assert!(p(vec![0, 3]) < p(vec![2, 3]));
    }
}
