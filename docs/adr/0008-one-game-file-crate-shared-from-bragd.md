# One crate of game file readers, kept in Bragd and used by coxswain

Supersedes [ADR 0001](0001-copy-game-code-from-coxswain.md).

Bragd and coxswain stay separate apps, but their game code is no longer copied: a fix made twice drifted, and coxswain's chess code kept moving ahead of Bragd's copy. The game code moves into Bragd in steps, each a crate of its own that coxswain then depends on:

| Step | Issue | What moves |
|---|---|---|
| 1 | [#387](https://github.com/mwo-dk/bragd/issues/387) | The game file readers: `crates/bragd-gamefiles` (PGN, SCID, ChessBase, Polyglot, SGF, KIF, CSA, DhtmlXQ, PDN, the openings list) |
| 2 | [#388](https://github.com/mwo-dk/bragd/issues/388) | The games table, the position index and the opening tree |
| 3 | [#389](https://github.com/mwo-dk/bragd/issues/389) | Play, review and engines |
| 4 | [#390](https://github.com/mwo-dk/bragd/issues/390) | Every crate coxswain's games module needs is on crates.io, with its own release flow |

After each step, coxswain switches to the crate in a pull request of its own and deletes its copy. The shared crates live in Bragd, because they are about games. A reader reads from a source it is handed (`Read + Seek`), never from a path: opening files, unpacking archives, texts and app folders stay with each app. Board logic comes only from the `rules-*` crates.

Coxswain is published on crates.io and cannot depend on a git repository, so the shared crates are published there too. `bragd-gamefiles` and the `rules-*` crates it uses (`rules-chess`, `rules-draughts`, `rules-go`, `rules-shogi`, `rules-xiangqi`) have versions of their own, apart from Bragd's, starting at 0.1.0, and are published before Bragd 1.0.0. This is the one exception to "nothing is released before 1.0.0". Only the owner publishes. Coxswain pins each with `=X.Y.Z`, so a new version reaches it only when it bumps the pin.

Once a step's crate exists, the code it holds changes only there: a fix to game-file reading is made in `bragd-gamefiles`, published, and taken up by coxswain through its pin. Coxswain's own copy is deleted, not kept beside the crate.

Coxswain ships on FreeBSD, NetBSD, OpenBSD, illumos and static musl as well as Linux, macOS and Windows. A pull request touching `crates/bragd-gamefiles` or a `rules-*` crate checks `bragd-gamefiles` for each of those targets in CI (`cargo check`, no linking).
