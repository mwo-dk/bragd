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
A filter on a Route of a Source that decides which of its Games are kept, for example rating 2000+ or slow time controls only. It applies to Games as they arrive and to what is already stored.
_Avoid_: filter (a Slice filters for viewing; a Strip rule removes), prune rule

**Route**:
A Strip rule paired with the Library its Games go to. A Source has one or more Routes. Its download is read once, and each Game lands in every Library whose Route keeps it.
_Avoid_: pipe, channel, destination

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

**Tip**:
An occasional one-line suggestion of a Feature the user has not used yet. It can be silenced one at a time, or all together.
_Avoid_: hint (a hint explains something where it applies), nag, notification

**Theme**:
The app's named appearance, such as Racing green, Nord or Aqua. Racing green is the default.
_Avoid_: mode, skin, dark mode (that is one kind of Theme)

**Look**:
The chrome style laid over a Theme, such as crt, win95 or aqua. Separate from the Theme's colours.
_Avoid_: skin, style

**Board theme**:
The four colours of a board: light square, dark square, highlight and marker. It is chosen independently of the Theme, or set to match it. It never controls how a board is built, because that belongs to the Game family.
_Avoid_: board colour, board skin

### Playing together

**Chat**:
Text messages between the two Players of a Game, open only when both agree.
_Avoid_: message, conversation

**Call**:
Voice, camera or both between two Players who are Friends, during a Game, open only when both agree. Nothing of it is recorded.
_Avoid_: video, stream, meeting

**Friend**:
Another player on the Bragd server whom you both accepted. Members of your Clubs can count as Friends.
_Avoid_: contact, buddy

**Bragd server**:
What Bragd's makers run for accounts, Friends, Games between Bragd players, and bringing Calls together: hosted Supabase plus Bragd's own game service.
_Avoid_: backend, cloud, lobby

### Clubs and competition

**Club**:
A named group of members who play together. Each member has a role (director, arbiter or member) and may carry IDs from FIDE, a national federation or Lichess, plus a rating kept by the club. A Club owns its Tournaments and its own ratings.
_Avoid_: group, society, team (a Team is what a Club fields in a League)

**Pairing system**:
The rule that decides who plays whom in each Round. The options are Swiss (FIDE Dutch), Monrad, Nordic Swiss, round-robin (Berger tables), knockout and Scheveningen.
_Avoid_: draw (a draw is a result), schedule

**Tournament**:
A competition a Club runs: its players, its Pairing system, its Rounds with their results, its tiebreaks and the log of every change made by hand. It is kept in the Club file.
_Avoid_: event, competition (alone), league (a League is between Teams)

**Round**:
One set of games of a Tournament, all paired at once. A player not paired in a Round has a bye or is away.
_Avoid_: game day, match (a match is between two Teams or two Engines)

**League**:
A team competition a Club runs: Teams meet each other in a round-robin, each match on the same number of boards. A match gives match points (2 for a win, 1 for a draw) from its board points, and the standings rank by match points, then board points. It is kept in the Club file.
_Avoid_: division (a name a League may have), team tournament

**Team**:
The players a Club fields in a League, in board order. Each match, a Team lines up as many of them as there are boards.
_Avoid_: squad, side (a Side is who moves)

**Tiebreak**:
A rule from FIDE's list (C.07), such as Buchholz or Sonneborn-Berger, that orders players level on points. A Tournament has its own ordered list of them.
_Avoid_: tie-break score, korrektion

**Rating list**:
A published list of players with their federation IDs and ratings, such as the FIDE list or the Danish Chess Union's (DSU) list. Bragd downloads it like a Source and uses it to find and update members.
_Avoid_: player database, ranking

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

**Edition**:
One exact build of an Engine: its version, its network where it has one, and its image digest when it runs in a container. Two Editions of the same Engine can judge a position differently.
_Avoid_: release, version (alone)

### Analysis

**Evaluation**:
One Edition's verdict on one position: its score, the depth it reached and its best line.
_Avoid_: eval, analysis, score (alone)

**Annotation**:
The structured judgement of a move: a mark such as ?? or !, the better move, and named motifs such as "hangs a piece". Bragd words it in the reader's language from its own texts.
_Avoid_: comment, NAG

**Commentary**:
Prose about a position or a move, written by a language model in one language and labelled as machine-written.
_Avoid_: annotation, notes

**Comment**:
Text a person wrote into a Game, as it came from its file. A translation of it is shown beside it and never replaces it.
_Avoid_: annotation, commentary

**Position store**:
The one store on a machine that keeps Evaluations, Annotations and Commentary by position, for every Library, Foreign database and Game played. Deleting a Library does not touch it.
_Avoid_: cache, evaluation database

**Peer**:
Another of your own machines whose Helper is paired with yours. Peers swap what is new in their Position stores and share the background analysis.
_Avoid_: node, remote, server, client

## Flagged ambiguities

**"Light" and "dark"**: these name both a square colour (a rule of the game) and an appearance. Always say which one: "dark square" or "dark theme".
