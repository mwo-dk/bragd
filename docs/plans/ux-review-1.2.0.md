# UX review before 1.2.0, 3 November 2026

A review of master at `b52e97a` as a user sees it, with the weight on what changed since the [1.0.0 review](ux-review-1.0.0.md): Sources (Lichess database, FICS, PGN Mentor, KGS), Play with Friends (dice, clocks, ratings, seeks, live moves, backgammon checker by checker, Delete my account), the Lichess tablebase hint and the install wizard's Knowledge step, keyboard play and Play's keys, the Chat filter and the Rules lessons. The release build ran in the screenshot sandbox: `ui-check.py --shots` over every main screen at 390 × 844, 1100 × 760 and 1100 × 700, and `pages.py` for `friends`, `sources`, `source-pages`, `keyboard`, `help`, `wizard` and `extras`. Every picture was read.

The UI check found no fault. The faults below are the ones it cannot see.

Severity: **H** high, **M** medium, **L** low.

## Findings, most severe first

| Id | Sev | Where | What a user meets | Status |
|---|---|---|---|---|
| V1 | M | Play with Friends, backgammon | **Offer a draw** in a Game that has no draw; the server's referee ends it by agreement. Ur and senet the same. | #344 |
| V2 | M | Play with Friends, backgammon | **Throw** is a plain button between *Resign a backgammon* and *Resign*, with no key; on Play it is the primary button with **T**. | #346 |
| V3 | M | Install wizard, Knowledge | *You can change this later in Settings → Knowledge*, but its Sources are under Databases → Sources and the Lichess tablebase switch under Settings → Online. | #348 |
| V4 | M | Rules, shogi | The lessons say Black and White; the board and the status line say Sente and Gote. | #347 |
| V5 | L | Play with Friends, backgammon | A plain **Resign** beside *Resign a single game*, *a gammon* and *a backgammon*, with nothing saying how it differs. | #345 |
| V6 | L | Delete my account | **Delete my account for good** was the primary button and Cancel the plain one, the other way round from removing a Source. | Fixed here. |
| V7 | L | Tablebases page, Settings → Online, wizard | One switch, two names: *Ask the Lichess tablebase for positions your tables do not cover* and *Ask Lichess for tablebase positions*. Two docs pages gave its default as on. | Fixed here: one key; the docs say off. |
| V8 | L | Install wizard, Knowledge | *Coming later: LumbrasGigaBase, OGS archive, …* stood under *More lists*, as if they were rating lists. | Fixed here: under the Sources. |
| V9 | L | Sources | The address line shows templates: `twic{issue}g.zip`, `{yyyy}-{mm}`. | #349 |
| V10 | L | Play with Friends, New Game | *Shogi 1480?*: the `?` of a provisional rating is explained only in the docs. | #350 |
| V11 | L | Screenshots | `pages.py source-pages` stopped with *no button 'Options'* when the Helper filled Sources after the fixed wait, so `sources-pgnmentor-*` still said *a month*. | Fixed here: `click` tries for 10 s; retaken. |
| V12 | L | Screenshots | `wizard-knowledge-nord-1100.png` was made by no script and shown nowhere; `wizard-welcome-he-1100`, `navigation-*`, `settings-knowledge-*` and `tablebases-*` were shown but made by no script (the navigation ones in the light Theme, against the Esc label in their text). | Fixed here: the first removed, the others in `pages.py wizard` and `extras` and retaken. |
| V13 | L | Screenshots | `slice-commentary-*` is made by no script. | #351 |
| V14 | L | Screenshot sandbox | Settings → Language model found a model server running on the host, as the sandbox shares 127.0.0.1. | #352 |

## Checked and clean

- Every Family's Play page at 390 and 1100 px with the right board, keys on New game, Undo, Flip board, Throw and Pass, and the keyboard ring on Play (`keyboard-390.png`).
- Sources for chess and go: each Source off until turned on, its size shown first, PGN Mentor and KGS saying *the first time, then only what is new*, What to fetch with its parts ticked.
- Play with Friends: sign-in, Friends, Clubs, seeks with clock and Rated, challenges naming clock and rating, Chat that both must allow, Calls, Delete my account typed to confirm, the backgammon throw and one checker moved with Undo.
- Settings → Online keeps *Ask Lichess for tablebase positions* off, says what is sent and where, and says Play with Friends stays hidden with no server address.
- The Rules lessons read as prose, show the pieces in hand for shogi, and name their move as the move list does.
- Wording follows the glossary: Game family, Variant, Library, Source, Route, Strip rule, Helper, Engine, Friend, Chat, Call, Bragd server.
