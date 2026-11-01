//! Maximum-weight matching in a general graph (Edmonds' blossom algorithm with
//! Galil's dual updates, O(n³)).
//!
//! Among the matchings of largest size it returns one of largest total weight.
//! Weights are doubled inside, so with integer weights every dual variable stays
//! an integer and no rounding enters.

use crate::wide::Wide;
use std::ops::{Add, Sub};

/// What a weight must support.
pub trait Weight: Copy + Ord + Add<Output = Self> + Sub<Output = Self> {
    const ZERO: Self;
    fn double(self) -> Self {
        self + self
    }
    /// Halves an even value.
    fn half(self) -> Self;
}

impl Weight for i64 {
    const ZERO: i64 = 0;
    fn half(self) -> i64 {
        self / 2
    }
}

impl Weight for Wide {
    const ZERO: Wide = Wide::ZERO;
    fn half(self) -> Wide {
        Wide::half(self)
    }
}

const NONE: usize = usize::MAX;

/// Returns `mate[v]`: the vertex matched to `v`, or `None`.
pub fn max_weight_matching<W: Weight>(n: usize, edges: &[(usize, usize, W)]) -> Vec<Option<usize>> {
    if edges.is_empty() || n == 0 {
        return vec![None; n];
    }
    let mut m = Solver::new(n, edges);
    m.solve();
    m.mate
        .iter()
        .map(|&p| if p == NONE { None } else { Some(m.endpoint[p]) })
        .collect()
}

struct Solver<W> {
    n: usize,
    edges: Vec<(usize, usize, W)>,
    endpoint: Vec<usize>,
    neighbend: Vec<Vec<usize>>,
    mate: Vec<usize>,
    label: Vec<u8>,
    labelend: Vec<usize>,
    inblossom: Vec<usize>,
    blossomparent: Vec<usize>,
    blossomchilds: Vec<Vec<usize>>,
    blossombase: Vec<usize>,
    blossomendps: Vec<Vec<usize>>,
    bestedge: Vec<usize>,
    blossombestedges: Vec<Option<Vec<usize>>>,
    unusedblossoms: Vec<usize>,
    dualvar: Vec<W>,
    allowedge: Vec<bool>,
    queue: Vec<usize>,
}

/// Python-style index into a cyclic list: `j` may be negative.
fn at(v: &[usize], j: isize) -> usize {
    let len = v.len() as isize;
    v[(((j % len) + len) % len) as usize]
}

impl<W: Weight> Solver<W> {
    fn new(n: usize, input: &[(usize, usize, W)]) -> Self {
        let edges: Vec<(usize, usize, W)> =
            input.iter().map(|&(i, j, w)| (i, j, w.double())).collect();
        let mut endpoint = Vec::with_capacity(2 * edges.len());
        let mut neighbend = vec![Vec::new(); n];
        for (k, &(i, j, _)) in edges.iter().enumerate() {
            endpoint.push(i);
            endpoint.push(j);
            neighbend[i].push(2 * k + 1);
            neighbend[j].push(2 * k);
        }
        let maxweight = edges
            .iter()
            .map(|e| e.2)
            .fold(W::ZERO, |a, b| if b > a { b } else { a });
        let mut dualvar = vec![maxweight; n];
        dualvar.extend(std::iter::repeat_n(W::ZERO, n));
        let ne = edges.len();
        Solver {
            n,
            edges,
            endpoint,
            neighbend,
            mate: vec![NONE; n],
            label: vec![0; 2 * n],
            labelend: vec![NONE; 2 * n],
            inblossom: (0..n).collect(),
            blossomparent: vec![NONE; 2 * n],
            blossomchilds: vec![Vec::new(); 2 * n],
            blossombase: (0..n).chain(std::iter::repeat_n(NONE, n)).collect(),
            blossomendps: vec![Vec::new(); 2 * n],
            bestedge: vec![NONE; 2 * n],
            blossombestedges: vec![None; 2 * n],
            unusedblossoms: (n..2 * n).collect(),
            dualvar,
            allowedge: vec![false; ne],
            queue: Vec::new(),
        }
    }

    fn slack(&self, k: usize) -> W {
        let (i, j, w) = self.edges[k];
        self.dualvar[i] + self.dualvar[j] - w.double()
    }

    fn leaves(&self, b: usize, out: &mut Vec<usize>) {
        if b < self.n {
            out.push(b);
        } else {
            for &t in &self.blossomchilds[b] {
                self.leaves(t, out);
            }
        }
    }

    fn leaves_of(&self, b: usize) -> Vec<usize> {
        let mut v = Vec::new();
        self.leaves(b, &mut v);
        v
    }

    fn assign_label(&mut self, w: usize, t: u8, p: usize) {
        let b = self.inblossom[w];
        self.label[w] = t;
        self.label[b] = t;
        self.labelend[w] = p;
        self.labelend[b] = p;
        self.bestedge[w] = NONE;
        self.bestedge[b] = NONE;
        if t == 1 {
            let l = self.leaves_of(b);
            self.queue.extend(l);
        } else {
            let base = self.blossombase[b];
            let mb = self.mate[base];
            self.assign_label(self.endpoint[mb], 1, mb ^ 1);
        }
    }

    fn scan_blossom(&mut self, mut v: usize, mut w: usize) -> usize {
        let mut path = Vec::new();
        let mut base = NONE;
        while v != NONE || w != NONE {
            let mut b = self.inblossom[v];
            if self.label[b] & 4 != 0 {
                base = self.blossombase[b];
                break;
            }
            path.push(b);
            self.label[b] = 5;
            if self.labelend[b] == NONE {
                v = NONE;
            } else {
                v = self.endpoint[self.labelend[b]];
                b = self.inblossom[v];
                v = self.endpoint[self.labelend[b]];
            }
            if w != NONE {
                std::mem::swap(&mut v, &mut w);
            }
        }
        for b in path {
            self.label[b] = 1;
        }
        base
    }

    fn add_blossom(&mut self, base: usize, k: usize) {
        let (mut v, mut w, _) = self.edges[k];
        let bb = self.inblossom[base];
        let mut bv = self.inblossom[v];
        let mut bw = self.inblossom[w];
        let b = self.unusedblossoms.pop().expect("blossom slots");
        self.blossombase[b] = base;
        self.blossomparent[b] = NONE;
        self.blossomparent[bb] = b;
        let mut path = Vec::new();
        let mut endps = Vec::new();
        while bv != bb {
            self.blossomparent[bv] = b;
            path.push(bv);
            endps.push(self.labelend[bv]);
            v = self.endpoint[self.labelend[bv]];
            bv = self.inblossom[v];
        }
        path.push(bb);
        path.reverse();
        endps.reverse();
        endps.push(2 * k);
        while bw != bb {
            self.blossomparent[bw] = b;
            path.push(bw);
            endps.push(self.labelend[bw] ^ 1);
            w = self.endpoint[self.labelend[bw]];
            bw = self.inblossom[w];
        }
        self.label[b] = 1;
        self.labelend[b] = self.labelend[bb];
        self.dualvar[b] = W::ZERO;
        self.blossomchilds[b] = path.clone();
        self.blossomendps[b] = endps;
        for v in self.leaves_of(b) {
            if self.label[self.inblossom[v]] == 2 {
                self.queue.push(v);
            }
            self.inblossom[v] = b;
        }
        let mut bestedgeto = vec![NONE; 2 * self.n];
        for &bv in &path {
            let nblists: Vec<Vec<usize>> = match self.blossombestedges[bv].take() {
                Some(list) => vec![list],
                None => self
                    .leaves_of(bv)
                    .into_iter()
                    .map(|v| self.neighbend[v].iter().map(|p| p / 2).collect())
                    .collect(),
            };
            for nblist in nblists {
                for k in nblist {
                    let (mut i, mut j, _) = self.edges[k];
                    if self.inblossom[j] == b {
                        std::mem::swap(&mut i, &mut j);
                    }
                    let _ = i;
                    let bj = self.inblossom[j];
                    if bj != b
                        && self.label[bj] == 1
                        && (bestedgeto[bj] == NONE || self.slack(k) < self.slack(bestedgeto[bj]))
                    {
                        bestedgeto[bj] = k;
                    }
                }
            }
            self.bestedge[bv] = NONE;
        }
        let list: Vec<usize> = bestedgeto.into_iter().filter(|&k| k != NONE).collect();
        self.bestedge[b] = NONE;
        for &k in &list {
            if self.bestedge[b] == NONE || self.slack(k) < self.slack(self.bestedge[b]) {
                self.bestedge[b] = k;
            }
        }
        self.blossombestedges[b] = Some(list);
    }

    fn expand_blossom(&mut self, b: usize, endstage: bool) {
        let childs = self.blossomchilds[b].clone();
        for &s in &childs {
            self.blossomparent[s] = NONE;
            if s < self.n {
                self.inblossom[s] = s;
            } else if endstage && self.dualvar[s] == W::ZERO {
                self.expand_blossom(s, endstage);
            } else {
                for v in self.leaves_of(s) {
                    self.inblossom[v] = s;
                }
            }
        }
        if !endstage && self.label[b] == 2 {
            let entrychild = self.inblossom[self.endpoint[self.labelend[b] ^ 1]];
            let len = childs.len() as isize;
            let mut j = childs.iter().position(|&c| c == entrychild).unwrap() as isize;
            let (jstep, endptrick): (isize, usize) = if j & 1 == 1 {
                j -= len;
                (1, 0)
            } else {
                (-1, 1)
            };
            let endps = self.blossomendps[b].clone();
            let mut p = self.labelend[b];
            while j != 0 {
                let q = at(&endps, j - endptrick as isize);
                self.label[self.endpoint[p ^ 1]] = 0;
                self.label[self.endpoint[q ^ endptrick ^ 1]] = 0;
                self.assign_label(self.endpoint[p ^ 1], 2, p);
                self.allowedge[q / 2] = true;
                j += jstep;
                p = at(&endps, j - endptrick as isize) ^ endptrick;
                self.allowedge[p / 2] = true;
                j += jstep;
            }
            let bv = at(&childs, j);
            let e = self.endpoint[p ^ 1];
            self.label[e] = 2;
            self.label[bv] = 2;
            self.labelend[e] = p;
            self.labelend[bv] = p;
            self.bestedge[bv] = NONE;
            j += jstep;
            while at(&childs, j) != entrychild {
                let bv = at(&childs, j);
                if self.label[bv] == 1 {
                    j += jstep;
                    continue;
                }
                let mut found = NONE;
                for v in self.leaves_of(bv) {
                    if self.label[v] != 0 {
                        found = v;
                        break;
                    }
                }
                if found != NONE {
                    let v = found;
                    self.label[v] = 0;
                    let mb = self.mate[self.blossombase[bv]];
                    self.label[self.endpoint[mb]] = 0;
                    self.assign_label(v, 2, self.labelend[v]);
                }
                j += jstep;
            }
            let _ = len;
        }
        self.label[b] = 0;
        self.labelend[b] = NONE;
        self.blossomchilds[b].clear();
        self.blossomendps[b].clear();
        self.blossombase[b] = NONE;
        self.blossombestedges[b] = None;
        self.bestedge[b] = NONE;
        self.unusedblossoms.push(b);
    }

    fn augment_blossom(&mut self, b: usize, v: usize) {
        let mut t = v;
        while self.blossomparent[t] != b {
            t = self.blossomparent[t];
        }
        if t >= self.n {
            self.augment_blossom(t, v);
        }
        let childs = self.blossomchilds[b].clone();
        let endps = self.blossomendps[b].clone();
        let len = childs.len() as isize;
        let i = childs.iter().position(|&c| c == t).unwrap();
        let mut j = i as isize;
        let (jstep, endptrick): (isize, usize) = if j & 1 == 1 {
            j -= len;
            (1, 0)
        } else {
            (-1, 1)
        };
        while j != 0 {
            j += jstep;
            let t = at(&childs, j);
            let p = at(&endps, j - endptrick as isize) ^ endptrick;
            if t >= self.n {
                self.augment_blossom(t, self.endpoint[p]);
            }
            j += jstep;
            let t = at(&childs, j);
            if t >= self.n {
                self.augment_blossom(t, self.endpoint[p ^ 1]);
            }
            self.mate[self.endpoint[p]] = p ^ 1;
            self.mate[self.endpoint[p ^ 1]] = p;
        }
        self.blossomchilds[b].rotate_left(i);
        self.blossomendps[b].rotate_left(i);
        self.blossombase[b] = self.blossombase[self.blossomchilds[b][0]];
    }

    fn augment_matching(&mut self, k: usize) {
        let (v, w, _) = self.edges[k];
        for (mut s, mut p) in [(v, 2 * k + 1), (w, 2 * k)] {
            loop {
                let bs = self.inblossom[s];
                if bs >= self.n {
                    self.augment_blossom(bs, s);
                }
                self.mate[s] = p;
                if self.labelend[bs] == NONE {
                    break;
                }
                let t = self.endpoint[self.labelend[bs]];
                let bt = self.inblossom[t];
                s = self.endpoint[self.labelend[bt]];
                let j = self.endpoint[self.labelend[bt] ^ 1];
                if bt >= self.n {
                    self.augment_blossom(bt, j);
                }
                self.mate[j] = self.labelend[bt];
                p = self.labelend[bt] ^ 1;
            }
        }
    }

    fn solve(&mut self) {
        let n = self.n;
        for _ in 0..n {
            self.label.iter_mut().for_each(|l| *l = 0);
            self.bestedge.iter_mut().for_each(|e| *e = NONE);
            for b in n..2 * n {
                self.blossombestedges[b] = None;
            }
            self.allowedge.iter_mut().for_each(|a| *a = false);
            self.queue.clear();
            for v in 0..n {
                if self.mate[v] == NONE && self.label[self.inblossom[v]] == 0 {
                    self.assign_label(v, 1, NONE);
                }
            }
            let mut augmented = false;
            loop {
                while let Some(v) = self.queue.pop() {
                    for idx in 0..self.neighbend[v].len() {
                        let p = self.neighbend[v][idx];
                        let k = p / 2;
                        let w = self.endpoint[p];
                        if self.inblossom[v] == self.inblossom[w] {
                            continue;
                        }
                        let mut kslack = W::ZERO;
                        if !self.allowedge[k] {
                            kslack = self.slack(k);
                            if kslack <= W::ZERO {
                                self.allowedge[k] = true;
                            }
                        }
                        if self.allowedge[k] {
                            if self.label[self.inblossom[w]] == 0 {
                                self.assign_label(w, 2, p ^ 1);
                            } else if self.label[self.inblossom[w]] == 1 {
                                let base = self.scan_blossom(v, w);
                                if base != NONE {
                                    self.add_blossom(base, k);
                                } else {
                                    self.augment_matching(k);
                                    augmented = true;
                                    break;
                                }
                            } else if self.label[w] == 0 {
                                self.label[w] = 2;
                                self.labelend[w] = p ^ 1;
                            }
                        } else if self.label[self.inblossom[w]] == 1 {
                            let b = self.inblossom[v];
                            if self.bestedge[b] == NONE || kslack < self.slack(self.bestedge[b]) {
                                self.bestedge[b] = k;
                            }
                        } else if self.label[w] == 0
                            && (self.bestedge[w] == NONE || kslack < self.slack(self.bestedge[w]))
                        {
                            self.bestedge[w] = k;
                        }
                    }
                    if augmented {
                        break;
                    }
                }
                if augmented {
                    break;
                }
                // Maximum cardinality: no delta of type 1 until nothing else is left.
                let mut deltatype = 0u8;
                let mut delta = W::ZERO;
                let mut deltaedge = NONE;
                let mut deltablossom = NONE;
                for v in 0..n {
                    if self.label[self.inblossom[v]] == 0 && self.bestedge[v] != NONE {
                        let d = self.slack(self.bestedge[v]);
                        if deltatype == 0 || d < delta {
                            delta = d;
                            deltatype = 2;
                            deltaedge = self.bestedge[v];
                        }
                    }
                }
                for b in 0..2 * n {
                    if self.blossomparent[b] == NONE
                        && self.label[b] == 1
                        && self.bestedge[b] != NONE
                    {
                        let d = self.slack(self.bestedge[b]).half();
                        if deltatype == 0 || d < delta {
                            delta = d;
                            deltatype = 3;
                            deltaedge = self.bestedge[b];
                        }
                    }
                }
                for b in n..2 * n {
                    if self.blossombase[b] != NONE
                        && self.blossomparent[b] == NONE
                        && self.label[b] == 2
                        && (deltatype == 0 || self.dualvar[b] < delta)
                    {
                        delta = self.dualvar[b];
                        deltatype = 4;
                        deltablossom = b;
                    }
                }
                if deltatype == 0 {
                    deltatype = 1;
                    let m = self.dualvar[..n].iter().copied().min().unwrap();
                    delta = if m > W::ZERO { m } else { W::ZERO };
                }
                for v in 0..n {
                    match self.label[self.inblossom[v]] {
                        1 => self.dualvar[v] = self.dualvar[v] - delta,
                        2 => self.dualvar[v] = self.dualvar[v] + delta,
                        _ => {}
                    }
                }
                for b in n..2 * n {
                    if self.blossombase[b] != NONE && self.blossomparent[b] == NONE {
                        match self.label[b] {
                            1 => self.dualvar[b] = self.dualvar[b] + delta,
                            2 => self.dualvar[b] = self.dualvar[b] - delta,
                            _ => {}
                        }
                    }
                }
                match deltatype {
                    1 => break,
                    2 => {
                        self.allowedge[deltaedge] = true;
                        let (mut i, mut j, _) = self.edges[deltaedge];
                        if self.label[self.inblossom[i]] == 0 {
                            std::mem::swap(&mut i, &mut j);
                        }
                        self.queue.push(i);
                    }
                    3 => {
                        self.allowedge[deltaedge] = true;
                        let (i, _, _) = self.edges[deltaedge];
                        self.queue.push(i);
                    }
                    _ => self.expand_blossom(deltablossom, false),
                }
            }
            if !augmented {
                break;
            }
            for b in n..2 * n {
                if self.blossomparent[b] == NONE
                    && self.blossombase[b] != NONE
                    && self.label[b] == 1
                    && self.dualvar[b] == W::ZERO
                {
                    self.expand_blossom(b, true);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Best (size, weight) over every matching, by brute force.
    fn brute(n: usize, edges: &[(usize, usize, i64)]) -> (usize, i64) {
        fn go(
            v: usize,
            n: usize,
            used: &mut [bool],
            adj: &[Vec<(usize, i64)>],
            acc: (usize, i64),
            best: &mut (usize, i64),
        ) {
            if v == n {
                if acc > *best {
                    *best = acc;
                }
                return;
            }
            if used[v] {
                return go(v + 1, n, used, adj, acc, best);
            }
            go(v + 1, n, used, adj, acc, best);
            used[v] = true;
            for &(u, w) in &adj[v] {
                if !used[u] {
                    used[u] = true;
                    go(v + 1, n, used, adj, (acc.0 + 1, acc.1 + w), best);
                    used[u] = false;
                }
            }
            used[v] = false;
        }
        let mut adj = vec![Vec::new(); n];
        for &(i, j, w) in edges {
            adj[i].push((j, w));
            adj[j].push((i, w));
        }
        let mut best = (0, i64::MIN);
        go(0, n, &mut vec![false; n], &adj, (0, 0), &mut best);
        best
    }

    fn score(mate: &[Option<usize>], edges: &[(usize, usize, i64)]) -> (usize, i64) {
        let mut size = 0;
        let mut w = 0;
        for &(i, j, wt) in edges {
            if mate[i] == Some(j) {
                assert_eq!(mate[j], Some(i));
                size += 1;
                w += wt;
            }
        }
        (size, w)
    }

    /// A small deterministic generator, so a failure can be replayed.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, m: u64) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 33) % m
        }
    }

    #[test]
    fn agrees_with_brute_force_on_random_graphs() {
        let mut rng = Lcg(7);
        for case in 0..3000 {
            let n = 2 + rng.next(9) as usize;
            let density = 1 + rng.next(9);
            let mut edges = Vec::new();
            for i in 0..n {
                for j in i + 1..n {
                    if rng.next(10) < density {
                        let w = rng.next(30) as i64 - if case % 3 == 0 { 15 } else { 0 };
                        edges.push((i, j, w));
                    }
                }
            }
            let mate = max_weight_matching(n, &edges);
            assert_eq!(
                score(&mate, &edges),
                brute(n, &edges),
                "case {case}: n={n} {edges:?}"
            );
        }
    }

    #[test]
    fn wide_weights_give_the_same_matching_value() {
        let mut rng = Lcg(11);
        for _ in 0..300 {
            let n = 2 + rng.next(9) as usize;
            let mut edges = Vec::new();
            for i in 0..n {
                for j in i + 1..n {
                    if rng.next(3) > 0 {
                        edges.push((i, j, rng.next(50) as i64));
                    }
                }
            }
            let wide: Vec<_> = edges
                .iter()
                .map(|&(i, j, w)| (i, j, Wide::shifted(w as u64, 300)))
                .collect();
            let mate = max_weight_matching(n, &wide);
            assert_eq!(score(&mate, &edges), brute(n, &edges));
        }
    }

    #[test]
    fn blossoms_are_handled() {
        // A triangle with a tail: the odd cycle must shrink for the best answer.
        let edges = [
            (0, 1, 8),
            (1, 2, 9),
            (0, 2, 10),
            (2, 3, 7),
            (3, 4, 2),
            (4, 5, 6),
        ];
        let mate = max_weight_matching(6, &edges);
        assert_eq!(score(&mate, &edges), brute(6, &edges));
    }
}
