//! A turn, and how it is written down.
//!
//! **A turn is not a move**, and that is the structural difference between this
//! engine and every other one in Bragd. Two dice give up to two moves and a
//! double gives four, and the whole turn is legal or illegal *together*: you
//! must play as many dice as any legal sequence can, so a move that is fine on
//! its own can be forbidden because taking it strands the other die. Generating
//! single moves and letting the caller pick one produces a legal-looking
//! sequence that is against the rules, which is why [`crate::Position`]
//! generates whole [`Turn`]s and this module has no "move" type at all.
//!
//! The notation is backgammon's own — `8/5 6/5`, `bar/20`, `6/off`, `13/11*` for
//! a hit — and it is used rather than invented. Unlike tafl this game
//! has a convention that every player and every match file already speaks, so
//! there is nothing to decide and a record written in it can be read by
//! somebody who has never seen Bragd.
//!
//! One thing about the numbers, because it is the mistake a reader makes once:
//! **they are the mover's own point numbers**. `13/11` is the mover's 13-point
//! to the mover's 11-point, and the opponent calls those the 12- and 14-points.

use core::fmt;

/// Where a checker starts.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Origin {
    /// On the bar, waiting to enter. A player with one here must enter it before
    /// playing anything else.
    Bar,
    /// The mover's own point number, 1 to 24.
    Point(u8),
}

/// Where it lands.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Destination {
    /// The mover's own point number, 1 to 24.
    Point(u8),
    /// Borne off. Out of the game and counted at the end.
    Off,
}

/// One checker moved one die's worth.
///
/// **The die is not stored, the origin and destination are.** It looks like the
/// redundant choice — a die is one byte and the destination is derivable from
/// it — and it is the other way round: bearing off from the 4-point with a roll
/// of 6-4 can use either die, so a play recorded as "a 6 from the 4-point" and
/// one recorded as "a 4 from the 4-point" are the same play written twice.
/// Recording where the checker went makes the notation round-trip exactly, which
/// recording the die does not.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Play {
    pub from: Origin,
    pub to: Destination,
    /// Whether an opposing blot stood on the destination and went to the bar.
    /// Derived when the play is generated, not supplied — see
    /// [`crate::Position::after_play`].
    pub hit: bool,
}

impl Play {
    pub fn new(from: Origin, to: Destination) -> Play {
        Play {
            from,
            to,
            hit: false,
        }
    }

    /// The pair that identifies the play, without the `*`.
    ///
    /// Two spellings of the same turn can disagree about which of two plays
    /// carried the hit — land two checkers on one enemy blot and whichever went
    /// first did the hitting — so the hit is not part of a play's identity.
    pub fn placement(self) -> (Origin, Destination) {
        (self.from, self.to)
    }
}

impl fmt::Display for Play {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.from {
            Origin::Bar => f.write_str("bar")?,
            Origin::Point(number) => write!(f, "{number}")?,
        }
        f.write_str("/")?;
        match self.to {
            Destination::Point(number) => write!(f, "{number}")?,
            Destination::Off => f.write_str("off")?,
        }
        if self.hit {
            f.write_str("*")?;
        }
        Ok(())
    }
}

/// What a player does with a roll: up to four plays, taken together.
///
/// A turn with no plays is not an error and not a missing turn — it is the
/// forced pass a player makes when a shut-out board leaves them nothing legal.
/// It is written `(no play)`, which is what match files write.
#[derive(Clone, PartialEq, Eq, Debug, Hash, Default)]
pub struct Turn {
    plays: Vec<Play>,
}

/// What a turn is written as when there is nothing to play.
pub const NO_PLAY: &str = "(no play)";

impl Turn {
    pub fn new(plays: Vec<Play>) -> Turn {
        Turn { plays }
    }

    /// The forced pass.
    pub fn pass() -> Turn {
        Turn { plays: Vec::new() }
    }

    pub fn plays(&self) -> &[Play] {
        &self.plays
    }

    pub fn is_pass(&self) -> bool {
        self.plays.is_empty()
    }

    /// How many dice this turn used.
    pub fn dice_used(&self) -> usize {
        self.plays.len()
    }

    /// The turn's identity: its plays, sorted, with the hits dropped.
    ///
    /// Two turns with the same plays in a different order reach the same
    /// position — the counts on every point are the same however you order the
    /// moves — so they are one turn written two ways, and a player who spells it
    /// the other way has not made an illegal move. This is what generation
    /// deduplicates on and what [`crate::Game::play`] matches against.
    pub fn placements(&self) -> Vec<(Origin, Destination)> {
        let mut placements: Vec<(Origin, Destination)> =
            self.plays.iter().map(|play| play.placement()).collect();
        placements.sort_unstable();
        placements
    }

    /// Reads backgammon's notation, including the shorthands real match files
    /// use: `8/5(2)` for the same play twice, and `13/7/5` for one checker
    /// moving twice. Both are expanded, so what comes back out is always the
    /// long form.
    pub fn parse(text: &str) -> Option<Turn> {
        let text = text.trim();
        if text == NO_PLAY {
            return Some(Turn::pass());
        }
        let mut plays = Vec::new();
        for token in text.split_whitespace() {
            parse_token(token, &mut plays)?;
        }
        // An empty string is not a pass. A pass has a spelling, and accepting
        // silence as one would turn a truncated message into a legal turn.
        if plays.is_empty() {
            return None;
        }
        Some(Turn::new(plays))
    }
}

/// One whitespace-separated token: a chain of steps with an optional `(n)`.
fn parse_token(token: &str, plays: &mut Vec<Play>) -> Option<()> {
    let (body, repeat) = match token.strip_suffix(')') {
        Some(rest) => {
            let (body, count) = rest.rsplit_once('(')?;
            let count: usize = count.parse().ok()?;
            if count == 0 || count > 4 {
                return None;
            }
            (body, count)
        }
        None => (token, 1),
    };

    let steps: Vec<&str> = body.split('/').collect();
    if steps.len() < 2 {
        return None;
    }
    let mut chain = Vec::new();
    let mut from = parse_origin(steps[0])?;
    for (position, step) in steps[1..].iter().enumerate() {
        let hit = step.ends_with('*');
        let to = parse_destination(step.strip_suffix('*').unwrap_or(step))?;
        // A checker on the bar re-enters the board. It cannot go from the bar
        // to the tray without ever standing on a point.
        if from == Origin::Bar && to == Destination::Off {
            return None;
        }
        chain.push(Play { from, to, hit });
        from = match to {
            Destination::Point(number) => Origin::Point(number),
            // Nothing continues from off the board.
            Destination::Off if position + 2 == steps.len() => break,
            Destination::Off => return None,
        };
    }

    for _ in 0..repeat {
        plays.extend(chain.iter().copied());
    }
    Some(())
}

fn parse_origin(text: &str) -> Option<Origin> {
    if text == "bar" {
        return Some(Origin::Bar);
    }
    Some(Origin::Point(parse_number(text)?))
}

fn parse_destination(text: &str) -> Option<Destination> {
    if text == "off" {
        return Some(Destination::Off);
    }
    Some(Destination::Point(parse_number(text)?))
}

fn parse_number(text: &str) -> Option<u8> {
    let number: u8 = text.parse().ok()?;
    (1..=24).contains(&number).then_some(number)
}

impl fmt::Display for Turn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.plays.is_empty() {
            return f.write_str(NO_PLAY);
        }
        for (index, play) in self.plays.iter().enumerate() {
            if index > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{play}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(number: u8) -> Origin {
        Origin::Point(number)
    }

    fn to(number: u8) -> Destination {
        Destination::Point(number)
    }

    #[test]
    fn a_turn_round_trips_through_its_notation() {
        let turn = Turn::new(vec![
            Play::new(point(8), to(5)),
            Play {
                from: point(13),
                to: to(11),
                hit: true,
            },
        ]);
        assert_eq!(turn.to_string(), "8/5 13/11*");
        assert_eq!(Turn::parse(&turn.to_string()), Some(turn));
    }

    #[test]
    fn the_bar_and_the_tray_have_their_own_spellings() {
        let entering = Turn::new(vec![Play::new(Origin::Bar, to(20))]);
        assert_eq!(entering.to_string(), "bar/20");
        assert_eq!(Turn::parse("bar/20"), Some(entering));

        let off = Turn::new(vec![Play::new(point(6), Destination::Off)]);
        assert_eq!(off.to_string(), "6/off");
        assert_eq!(Turn::parse("6/off"), Some(off));
    }

    #[test]
    fn a_pass_is_written_down_rather_than_left_blank() {
        assert_eq!(Turn::pass().to_string(), NO_PLAY);
        assert_eq!(Turn::parse(NO_PLAY), Some(Turn::pass()));
        // Silence is not a pass: a truncated message must not read as a legal
        // turn.
        assert_eq!(Turn::parse(""), None);
    }

    #[test]
    fn the_shorthands_match_files_use_are_expanded() {
        // The same play twice.
        assert_eq!(
            Turn::parse("8/5(2)"),
            Some(Turn::new(vec![
                Play::new(point(8), to(5)),
                Play::new(point(8), to(5)),
            ]))
        );
        // One checker moving twice, with the hit on the intermediate point.
        assert_eq!(
            Turn::parse("13/7*/5"),
            Some(Turn::new(vec![
                Play {
                    from: point(13),
                    to: to(7),
                    hit: true
                },
                Play::new(point(7), to(5)),
            ]))
        );
        // And a chain that ends by bearing off.
        assert_eq!(
            Turn::parse("8/2/off"),
            Some(Turn::new(vec![
                Play::new(point(8), to(2)),
                Play::new(point(2), Destination::Off),
            ]))
        );
    }

    #[test]
    fn nonsense_does_not_parse() {
        assert_eq!(Turn::parse("13"), None, "a play needs two ends");
        assert_eq!(Turn::parse("25/20"), None, "there is no 25-point");
        assert_eq!(Turn::parse("0/off"), None, "nor a 0-point");
        assert_eq!(Turn::parse("6/off/2"), None, "nothing continues from off");
        assert_eq!(Turn::parse("bar/off"), None, "the bar is not the tray");
        assert_eq!(Turn::parse("8/5(0)"), None);
    }

    #[test]
    fn the_order_of_the_plays_is_not_part_of_the_turns_identity() {
        // Two spellings of one turn. They put the same checkers in the same
        // places, so a player who writes it the other way round has not made an
        // illegal move — which is why generation deduplicates on this and the
        // Game matches on it.
        let one = Turn::parse("8/5 6/5").expect("a turn");
        let other = Turn::parse("6/5 8/5").expect("a turn");
        assert_ne!(one, other);
        assert_eq!(one.placements(), other.placements());

        // And the hit is not part of it either, for the same reason: land two
        // checkers on one blot and whichever went first did the hitting.
        let hitting = Turn::parse("8/5* 6/5").expect("a turn");
        assert_eq!(one.placements(), hitting.placements());
    }
}
