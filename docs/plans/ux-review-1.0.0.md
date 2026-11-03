# UX review before 1.0.0, 3 November 2026

A review of master at `ebe6efa` as a user sees it. The release build ran in the screenshot sandbox: `ui-check.py --shots` over every main screen at 390 × 844, 1100 × 760 and 1100 × 700; `pages.py boards` for every Game family in every Board theme; and a review script with *Only chess*, *Only go* and a first start that picks *Only go* in the install wizard. Every picture was read. The judgement is against CLAUDE.md (*Nothing hidden*, *Settings decide what shows*, *Texts in every language*, *Nothing leaves the machine unless the user asks*), the glossary's words, clear errors, clicks per flow, keyboard reach and contrast.

The UI check found no fault: no text cut off, no page wider than its window, no axe-core contrast, label or focus fault, and every board fits a 700 px window. The faults below are the ones it cannot see.

Severity: **H** high, **M** medium, **L** low.

## Findings, most severe first

| Id | Sev | Where | What a user meets | Status |
|---|---|---|---|---|
| U1 | H | `gui/src/Play.svelte` (`sideways`) | On a window 760 px wide or less, the backgammon board is drawn about 100 px wide in the middle of the page. The rule that lays a long race track on its side (for Aseb) also caught backgammon, whose own board does not turn. The committed `play-backgammon-390.png` showed it. | Fixed here: `sideways()` in `board.js`, tested; the UI check now fails a Play board narrower than 70% of a narrow page. |
| U2 | H | `Board.svelte`, `BackgammonBoard.svelte` | No board can be played from the keyboard: every square has `tabindex="-1"` and there are no arrow keys. | #300 |
| U3 | M | `crates/bragd-core/src/features.rs` | F1 → Features listed every feature to everyone. With only go on, it offered *Play on Lichess*, *Play on Lishogi*, *Tablebases*, *Ur rule sets* and *Ur solved*. The Features already name their Families, which What's new and the Tips used. | Fixed here: `features::list` takes the Config; test added. |
| U4 | M | `tablebases.online` | Chess positions the local tables do not cover go to the Lichess tablebase API by default. CLAUDE.md names the update check as the only thing that leaves unasked. | #306 (a decision) |
| U5 | M | What's new, F1, Settings → Online | *Play with Friends*, *Chat with Bragd players* and *Calls with Friends* are announced, but the Bragd server address is empty until the server is up, so the page is hidden and nobody can reach them. | #307 |
| U6 | M | `crates/bragd-games/src/positions.rs` | The teaching positions on every Rules page are English in every language. | #302 |
| U7 | M | `gui/src/Rules.svelte` | A lesson about a drop (*The drop you may not make*) does not show the pieces in hand it talks about. | #303 |
| U8 | M | Play | New game, Undo, Flip board, Throw and Pass have no key, and none is shown in F1. | #301 |
| U9 | L | `gui/src-tauri/src/rules.rs` | A lesson named its move in the engine's wire form: *The move: d1d8* under a text that says *Rd8*. | Fixed here: the move list's writing (`Rd8#`), from the replayed Game; test added. |
| U10 | L | `positions.rs` | The shogi lesson showed raw Markdown, `**A pawn may not be dropped to give checkmate.**`. | Fixed here; a test refuses `**` in a lesson. |
| U11 | L | `gui/src/Rules.svelte` | The Rules page had no title; with the sidebar folded at 390 px it did not say which Family it was. | Fixed here: *Chess: Rules*, like *Chess: Engines*. |
| U12 | L | `gui/src/Play.svelte` (`fit`) | Ur and senet boards stay about a third of the room at 1100 px wide. | #304 |
| U13 | L | Sidebar | Each Family opened stays open, so Clubs, Settings, Help and What's new scroll out of a 760 px window after a few Families. | #305 |
| U14 | L | Chess → Databases → Sources | Five Sources are listed with *this version of Bragd cannot fetch it yet*. | #308 |
| U15 | L | `gui/src-tauri/src/helper_link.rs` | A Helper that cannot start shows *No such file or directory (os error 2)*, naming neither the program nor what to do. | #309 |
| U16 | L | Chess → Engines at 390 px | Variant names wrap mid-name (*Go* / *13×13*). | Left: readable, and the column cannot be wider at 390 px. |
| U17 | L | Docs | `docs/getting-started/help.md` listed 16 of the 35 F1 entries. | Fixed here: the table lists them all. |

Play on OGS at the end of the wizard's Online step for an *Only go* user says sign-in cannot start yet; that is #235.

## Checked and clean

- Every Family's board in each of the four Board themes (Racing green, Slate, Walnut, Match theme): squares, points and lines in the right colours, draughts on the dark squares, gote's shogi pieces turned, xiangqi's palace and river, tafl's throne and corners, the Ur rosettes and senet's marked houses.
- With *Only chess* or *Only go*: the sidebar lists that Family's items alone, New game, Settings → Online, the wizard's Engines, Knowledge and Online steps, What's new and Clubs' rating lists show only what concerns it.
- Settings → Game families keeps every Family's switch, so a Family turned off can be turned back on; its Variants show dimmed.
- Wording follows the glossary: Game family, Variant, Library, Source, Slice, Helper, Engine, Bot, Board theme.
- Errors for missing Engines say what is missing and where to get it.
