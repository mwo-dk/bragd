# Working on Bragd

These rules apply to every contributor and every Claude session in this repository. The design is in `docs/spec.md`, the vocabulary in `GLOSSARY.md`, and the decisions in `docs/adr/`. Read `BASELINE.md` before starting: it lists what must not be re-litigated and what is known broken.

## The shape of the app

- **Svelte renders; Rust decides.** Game rules, legal moves, notation, databases, engines, containers, settings, texts and the Helper all live in Rust crates. Svelte and TS only draw what Rust hands them and pass input back through Tauri commands. A rule, a move check or a parser written in TS fails review.
- **Rules crates have no dependencies** (`crates/rules/*`). They are pure Rust, so a wasm build stays possible. Each Game family owns its rules, its pieces and the look of its board. Families share nothing (`GLOSSARY.md`).
- **One artwork lineage.** All piece art comes from the set in `assets/pieces/`: Cburnett for chess, plus our own fairy, shogi, xiangqi, janggi, draughts, tafl and go pieces. Glyphs are drawn as paths, never as `<text>`, so no piece depends on installed fonts. Sprite symbol ids carry the Family name. New art matches the set, and its licence goes into `docs/CREDITS.md`.
- **Desktop only.** No web client and no wasm pipeline until the owner asks for one.
- **No names from other projects in code.** Code copied from elsewhere is renamed completely: crates, identifiers, environment variables, paths, strings and comments. Only the provenance docs (`docs/coxswain-sync.md`, `docs/adr/`) name a source project.

## Shipping

- **All changes go through pull requests.** `master` is protected and merges are squash only. Work on a branch in a worktree under `.worktrees/` (ignored by git), open the PR with `gh pr create`, and watch it until it merges (`gh pr checks <n> --watch`). When a check fails, read `gh run view <id> --log-failed`, fix it, push and watch again. A PR is done when it is merged, not when it is opened.
- **Commits:** one plain sentence saying what a user gets. No `Co-Authored-By` or other Claude attribution.
- **Feature and fix PRs never bump the version.** A user-visible change adds one file, `changelog.d/<branch>.md`, holding one table row: `| **What a user gets.** In plain words. | [Docs page](page.md#anchor) |`. A feature's row ends with ` <!-- feature --> <!-- tour: card-id -->`, naming its tour card.
- **A release is a PR of its own** on `release/X.Y.Z`, made by `packaging/release-pr.py`. It bumps the version (minor if any row is a feature, patch otherwise), folds the `changelog.d/` rows into `docs/changelog.md` and the README's last ten, and deletes the fragments.
  - Exit 3: the current version is unpublished.
  - Exit 4: the version's milestone still has open issues.
  - Exit 5: a feature row has no tour card in `crates/bragd-core/src/tour.rs`.
  **Nothing is released before 1.0.0.** Milestones 0.2.0 to 0.5.0 are steps towards it, and no release PR is made for them. From 1.0.0, releases run weekly on Sunday, with a rehearsal on Saturday. The nightly run tests every system whenever `master` changed.
- **Never a partial release.** Every installer, each with its `.sha256`, one `SHA256SUMS` over all of them, and every channel, or the release does not go out. No `continue-on-error` on anything that makes a release file. Channels: GitHub release, Homebrew cask, WinGet, Flatpak, Nix (`docs/spec.md`). A new build target goes into the matrix and into `packaging/release-assets.sh` together.
- **Done = milestone empty.** Every agreed piece of work is an issue in its version's milestone. Every cut or deferral becomes its own issue.
- **Dependabot PRs are not merged as they are.** Apply the update in a PR of our own, then close the Dependabot PR naming ours.
- **No legacy.** Dependencies stay current and known advisories get fixed. Old code paths, shims and dead options are removed, not carried along.
- **Keep up with the code we copied.** `docs/coxswain-sync.md` names the last upstream commit we synced from. The weekly sync issue lists what changed since then. Each item lands in a PR or is closed as not wanted, with a reason.

## Building and checking

- **GUI:** `cd gui && npm run build`, then `cargo build --release -p bragd-gui`, which embeds `gui/dist`. For the live dev server: `npx tauri dev -- --no-default-features`.
- **Check before pushing:** `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`. For GUI changes also `cd gui && npx svelte-check && npm test && npm run build`.
- **The UI check:** after a release build, `cd docs/screenshots && uv run --with websocket-client python ui-check.py [--shots DIR] [screen …]`. It opens every main screen at 390 × 844, 1100 × 760 and 1100 × 700 in the screenshot sandbox and fails on text that does not fit its box, a page wider than the window, axe-core's contrast, label and focus rules (axe-core comes from `gui/node_modules`, so `npm ci` first), and a board that does not fit a 700 px window. CI runs it in the *UI check* job; on a failure its pictures are an artifact.
- **A test must be able to fail.** A test that passes whatever the code does is deleted.
- **No clock in a test's proof.** A test proves "quick" by counting work, not by timing it. Time limits only guard against hangs.
- **When two copies disagree, delete one.** Do not add an abstraction to reconcile them.
- **Bills of materials:** a crate that does cryptography is described in `tools/bom/crypto.toml`, and every licence must be allowed by `deny.toml`.

## Every feature

- **Texts in every language.** User-facing strings go through `t!` / `t()`, with keys in `crates/bragd-core/locales/*.json`. The set of locales is fixed: do not add or remove one without the owner. The i18n test fails on a missing key. Settings has *Suggest a better translation*.
- **Nothing hidden.** Every capability is reachable from something visible: the sidebar, a menu, a button, or the command list. Its key is shown where it applies and in F1. Nothing exists only as a gesture or only as a `config.toml` key. What a user would not guess gets a hint, shown a few times. When something is off or needs something installed, a notice says so once.
- **Every feature has an entry in F1 → Features** (`bragd_core::features::FEATURES`), with its title, one line on what it does, and two or three questions with short answers, in every language.
- **Every user-facing feature ships a tour card.** Cards newer than the user's `seen_version` show after an upgrade, in the same frame as the install wizard. A `<!-- feature -->` row names its card with `<!-- tour: card-id -->`, and the id must be in `crates/bragd-core/src/tour.rs`, or the release gate fails (`packaging/README.md`).
- **Settings decide what shows.** A deselected Game family or Variant disappears from the sidebar, New game, the wizard and engine setup. Its files still open, and its data is never deleted.
- **Nothing leaves the machine unless the user asks.** The exception is the update check, which can be turned off. Downloads (Sources, tablebases, images) are opt-in, show their size first, and Settings names where they come from. Online tokens go in the OS keychain only.
- **Work a user's machine must redo is a decision.** A change that makes existing Libraries be re-indexed or re-deduplicated says so in the PR title and in its changelog row.

## Documentation

- The README is an overview. **Every feature is walked through under `docs/`**, one page per feature, in folders by area, each folder with a `README.md` index. A feature PR updates its docs in the same PR.
- **The docs check blocks merge** (`tools/docs-check.py`). It checks that links and anchors resolve, that every setting, action, flag and theme is documented, that every page has its Previous/Next links, and that a minor release has no screenshot placeholders left.
- **A change fixes every mention of the old way**: in docs, the README, the F1 texts and the pictures.
- Every feature page has *Questions*, using `####` headings for the real whys and hows.
- Plain British English: colour, licence, catalogue, grey, centre. Short concrete sentences, tables for keys and options, no marketing.

## Testing on a machine someone uses

- **A fake home, always.** App runs for tests or screenshots use a fake `HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME` and `XDG_DATA_HOME`. Never touch the owner's config, Libraries, engines or container images.
- **Nothing left behind.** Downloads and builds made for a task go in the session's scratch folder or `.worktrees/` and are removed afterwards.
- **Stop only what you started, by its own PID.** Never kill by name, pattern or process group.
- Never run podman or docker inside the screenshot sandbox.

## Screenshots

- Screenshots in `docs/screenshots/` are taken by `docs/screenshots/cap.py`. It runs the app in a bwrap sandbox with a fake home, on GTK Broadway, shown in a headless Chromium. The default Racing green Theme is used.
- **Look at it before calling it done.** After a UI change, capture the screen at 390 px and 1100 px wide and read the PNG. "It renders" is not proof: a board drawn wrong but without errors is our most common bug.
- Every Game family's board is captured in every Board theme.
- A PR that changes what a screen shows retakes that screen's pictures. Otherwise it marks each one `<!-- screenshot: name.png: what it must show -->` and says so in the PR.
- **GIF rules:**
  - at most 8 s, 800 px wide, 10–12 fps, under 1.5 MB, looping
  - a still `<name>.png` of the first frame next to it
  - shown through `<picture>` with `prefers-reduced-motion`

## Workers

Claude Code orchestrates. Mechanical steps may go to other workers: local models, OpenCode, Codex. They are routed through the user's `harness` skill, which keeps the routing, the caps and the statistics outside this repository. `AGENTS.md` is this file, so every worker reads the same rules. A worker's change is checked like any other: by tests, then by Claude reading the diff.
