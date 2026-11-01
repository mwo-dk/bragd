//! A game record's board: any size an SGF file allows, stones as written, nothing checked.
//!
//! This is not [`crate::Board`], whose array holds the playable sizes (up to 19 square) and
//! whose moves are tested for legality. A record can be rectangular, up to 52 by 52, hold
//! set-up stones anywhere, and every move in it is simply taken as played: a stone goes down
//! on any point, the other side's groups with no liberty come off, then its own (a suicide,
//! which some rules allow). There is no ko, no turn and no score.
//!
//! A made-up record can have huge groups, so the points looked at for liberties are counted:
//! a board costs at most [`MAX_LOOKED`] of them (about 50 ms), and past that stones are only
//! put down, never taken off.

/// The most points looked at for liberties by one board.
pub const MAX_LOOKED: u64 = 20_000_000;

/// A stone's colour on a point: 0 empty, 1 black, 2 white.
pub type Stone = u8;

/// A record's board and the stones taken so far.
#[derive(Clone, Debug)]
pub struct Record {
    pub width: u8,
    pub height: u8,
    /// Row by row from the top: 0 empty, 1 black, 2 white.
    pub stones: Vec<Stone>,
    /// Stones taken: by Black, by White.
    pub captures: [u32; 2],
    seen: Vec<bool>,
    budget: u64,
}

impl Record {
    pub fn new(width: u8, height: u8) -> Record {
        let n = width as usize * height as usize;
        Record { width, height, stones: vec![0; n], captures: [0, 0], seen: vec![false; n], budget: MAX_LOOKED }
    }

    /// Puts `stone` (0 takes the point's stone off) on a point, whatever is there. A point off
    /// the board is ignored.
    pub fn set(&mut self, x: u8, y: u8, stone: Stone) {
        if x < self.width && y < self.height {
            self.stones[y as usize * self.width as usize + x as usize] = stone;
        }
    }

    /// A stone of Black (else White) played at `x`, `y`: the other side's groups without a
    /// liberty are taken off, then its own. A point off the board is ignored.
    pub fn play(&mut self, black: bool, x: u8, y: u8) {
        if x >= self.width || y >= self.height {
            return;
        }
        let w = self.width as usize;
        let me = if black { 1 } else { 2 };
        let at = y as usize * w + x as usize;
        self.stones[at] = me;
        let mut taken = 0;
        for n in self.neighbours(at) {
            if self.stones[n] == 3 - me {
                taken += self.take(n);
            }
        }
        self.captures[(!black) as usize] += taken;
        if taken == 0 {
            let lost = self.take(at);
            self.captures[black as usize] += lost;
        }
    }

    fn neighbours(&self, i: usize) -> impl Iterator<Item = usize> + use<> {
        let (w, h) = (self.width as usize, self.height as usize);
        let (x, y) = (i % w, i / w);
        [(x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1), (y > 0).then(|| i - w), (y + 1 < h).then(|| i + w)].into_iter().flatten()
    }

    /// The group on `from` taken off if it has no liberty: how many stones.
    fn take(&mut self, from: usize) -> u32 {
        let colour = self.stones[from];
        let mut group = vec![from];
        let mut stack = vec![from];
        self.seen[from] = true;
        let mut free = false;
        // Done at its first liberty; out of budget counts as one.
        while !free && let Some(i) = stack.pop() {
            if self.budget == 0 {
                free = true;
                break;
            }
            self.budget -= 1;
            for n in self.neighbours(i) {
                match self.stones[n] {
                    0 => free = true,
                    c if c == colour && !self.seen[n] => {
                        self.seen[n] = true;
                        group.push(n);
                        stack.push(n);
                    }
                    _ => {}
                }
            }
        }
        for &i in &group {
            self.seen[i] = false;
        }
        if free {
            return 0;
        }
        for &i in &group {
            self.stones[i] = 0;
        }
        group.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stone_takes_the_group_it_fills_the_last_liberty_of() {
        let mut r = Record::new(5, 3);
        r.set(0, 0, 2);
        r.set(1, 0, 2);
        r.play(true, 2, 0);
        r.play(true, 0, 1);
        assert_eq!((r.stones[0], r.captures), (2, [0, 0]));
        r.play(true, 1, 1);
        assert_eq!((r.stones[0], r.stones[1], r.captures), (0, 0, [2, 0]));
    }

    #[test]
    fn a_suicide_is_allowed_and_counted_against_the_player() {
        let mut r = Record::new(3, 3);
        r.set(1, 0, 2);
        r.set(0, 1, 2);
        r.play(true, 0, 0);
        assert_eq!((r.stones[0], r.captures), (0, [0, 1]));
    }

    #[test]
    fn a_board_can_be_rectangular_up_to_fifty_two() {
        let mut r = Record::new(52, 7);
        r.set(51, 6, 2);
        r.play(true, 50, 6);
        r.play(true, 51, 5);
        assert_eq!((r.stones[6 * 52 + 51], r.captures), (0, [1, 0]));
        r.set(52, 0, 1);
        r.play(false, 0, 7);
        assert_eq!(r.stones.iter().filter(|&&s| s != 0).count(), 2, "points off the board are ignored");
    }

    #[test]
    fn past_its_budget_a_board_only_puts_stones_down() {
        let mut r = Record::new(3, 3);
        r.budget = 0;
        r.set(0, 0, 2);
        r.play(true, 1, 0);
        r.play(true, 0, 1);
        assert_eq!((r.stones[0], r.captures), (2, [0, 0]), "nothing is taken once the budget is gone");
    }
}
