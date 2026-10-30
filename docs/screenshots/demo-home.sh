#!/usr/bin/env bash
# The demo home the screenshots are taken in (sandbox.sh), made fresh: nothing of the owner's,
# nothing downloaded. Its Settings are the defaults, so the Racing green Theme and Board theme
# show. It holds a few games to fill a demo Library.
#
#   docs/screenshots/demo-home.sh <dir>
#
# TODO(demo-library): a stub until bragd-library can make a Library (big-bang S4). Then build
# ~/.local/share/bragd/Libraries/Demo from the games below with the library crate (an example
# binary, run here outside the sandbox with HOME and the XDG folders pointed at <dir>), so the
# screens open on a Library that has games in it.
set -euo pipefail
dir="${1:?usage: demo-home.sh <dir>}"
mkdir -p "$dir"/{.config,.cache,.local/share,Games}

# Three short games from the public domain (Morphy's Opera game, a Scholar's mate and a draw by
# repetition), in PGN, for the demo Library and the Foreign database screens.
cat > "$dir/Games/demo.pgn" <<'PGN'
[Event "Opera game"]
[Site "Paris"]
[Date "1858.11.02"]
[White "Paul Morphy"]
[Black "Duke Karl / Count Isouard"]
[Result "1-0"]

1. e4 e5 2. Nf3 d6 3. d4 Bg4 4. dxe5 Bxf3 5. Qxf3 dxe5 6. Bc4 Nf6 7. Qb3 Qe7
8. Nc3 c6 9. Bg5 b5 10. Nxb5 cxb5 11. Bxb5+ Nbd7 12. O-O-O Rd8 13. Rxd7 Rxd7
14. Rd1 Qe6 15. Bxd7+ Nxd7 16. Qb8+ Nxb8 17. Rd8# 1-0

[Event "Demo"]
[Site "Demo"]
[Date "2026.01.01"]
[White "White"]
[Black "Black"]
[Result "1-0"]

1. e4 e5 2. Bc4 Nc6 3. Qh5 Nf6 4. Qxf7# 1-0

[Event "Demo"]
[Site "Demo"]
[Date "2026.01.02"]
[White "White"]
[Black "Black"]
[Result "1/2-1/2"]

1. Nf3 Nf6 2. Ng1 Ng8 3. Nf3 Nf6 4. Ng1 Ng8 1/2-1/2
PGN

echo "demo home: $dir"
