# Bragd

A desktop app for playing, studying and keeping turn-based board games. Its vocabulary comes from the retired ../games site.

## Language

### Rules

**Game family**:
A group of games that share a lineage and most of their rules, such as chess, draughts, tafl or xiangqi. A family owns its rules, its pieces and the look of its board.
_Avoid_: game type, category

**Variant**:
One specific ruleset within a Game family. Standard chess is a Variant too, not the absence of one.
_Avoid_: mode, ruleset, flavour

**Side**:
One of the two seats in a Game, named the way its Variant names it: White and Black, Attackers and Defenders, Red and Black. It is not a colour.
_Avoid_: colour, player number

### Playing

**Game**:
A single contest between two Sides, from starting position to a result.
_Avoid_: match, session, board

**Player**:
Whoever holds one of the two Sides of a Game: a person, an Engine or a remote opponent. It is a role, not a kind of person.
_Avoid_: user, opponent

### Knowledge

**Foreign database**:
A game file in someone else's format, such as PGN, SCID, ChessBase, SGF, KIF or PDN. Bragd reads it in place. Overwriting a downloaded one is allowed but discouraged.
_Avoid_: import, external DB

**Library**:
Bragd's own consolidated store of Games. Imports land here, each Game appears once, and every Game remembers its Sources.
_Avoid_: database (ambiguous), collection, archive

**Slice**:
A named query over one or more Libraries, such as "Grünfeld" or "both players 2500+". It is live by default. As an option, it can be kept as a separate copy that can be edited.
_Avoid_: collection, filter, view

**Strip rule**:
A filter on a Source that decides which of its Games are kept, for example rating 2000+ or slow time controls only. It applies to Games as they arrive and to what is already stored.
_Avoid_: filter (a Slice filters for viewing; a Strip rule removes), prune rule

**Disk watch**:
The sizes of Libraries, downloads, Tablebases and images held against limits you set, with a warning when one is crossed.
_Avoid_: quota, disk monitor

**Source**:
Where Games come from, such as TWIC, the Lichess database or Chess Mentor. A Source can be fetched again on a schedule.
_Avoid_: feed, provider

**Duplicate**:
Two Games with the same moves from the same starting position, whatever their headers say.
_Avoid_: copy, clone

**Near-duplicate**:
A Game whose moves are a prefix of another Game's moves, such as a copy cut off early. It is flagged for review, never merged on its own.

**Helper**:
The background process that fetches Sources, indexes, and finds Duplicates. One Helper serves every open Bragd window.
_Avoid_: daemon, service, worker

### Presentation

**Theme**:
The app's named appearance, such as Racing green, Nord or Aqua. Racing green is the default.
_Avoid_: mode, skin, dark mode (that is one kind of Theme)

**Look**:
The chrome style laid over a Theme, such as crt, win95 or aqua. Separate from the Theme's colours.
_Avoid_: skin, style

**Board theme**:
The four colours of a board: light square, dark square, highlight and marker. It is chosen independently of the Theme, or set to match it. It never controls how a board is built, because that belongs to the Game family.
_Avoid_: board colour, board skin

### Engines

**Engine**:
A program that plays or evaluates positions for one or more Variants. It is either installed on the machine, run from a container Image, or built into Bragd.
_Avoid_: AI, bot (except for Bragd's own), computer

**Bot**:
An Engine built into Bragd, for Families that have no outside Engine (tafl, ur, senet).
_Avoid_: AI

**Tablebase**:
A precomputed table of perfect play for positions with few pieces.
_Avoid_: endgame database, EGTB

## Flagged ambiguities

**"Light" and "dark"**: these name both a square colour (a rule of the game) and an appearance. Always say which one: "dark square" or "dark theme".
