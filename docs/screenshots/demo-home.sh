#!/usr/bin/env bash
# The demo home the screenshots are taken in (sandbox.sh), made fresh: nothing of the owner's,
# nothing downloaded. Its Settings are the defaults, so the Racing green Theme and Board theme
# show. It holds a few games, and a demo Library made from them with the library crate
# (`bragd-library`'s `demo_library` example, run here outside the sandbox, writing only into
# <dir>): a Duplicate from two Sources, a Near-duplicate cut short, a Slice, and a player's Games.
# It also holds a few Games of every Family with an Engine (go, shogi, xiangqi, draughts and
# backgammon), and `fake-engine`, a stand-in engine for go, shogi, xiangqi and backgammon.
#
#   docs/screenshots/demo-home.sh <dir>
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

{[%evp 0,30,25,45,40,60,35,70,90,85,120,110,150,140,180,170,200,190,250,240,300,280,350,330,400,380,450,440,600,580,900,880,1000]}
1. e4 e5 2. Nf3 d6 {Philidor's Defence.} 3. d4 Bg4 4. dxe5 Bxf3 5. Qxf3 dxe5 6. Bc4 Nf6 7. Qb3 Qe7
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

# The Scholar's mate as another Source spells it (a Duplicate), and the Opera game cut off after
# ten moves (a Near-duplicate).
cat > "$dir/Games/copies.pgn" <<'PGN'
[Event "Demo copy"]
[Site "?"]
[Date "2026.01.01"]
[White "W. Hite"]
[Black "B. Lack"]
[WhiteElo "1500"]
[Result "1-0"]

1. e4 e5 2. Bc4 Nc6 3. Qh5 Nf6 4. Qxf7# 1-0

[Event "Opera game, first moves"]
[Site "Paris"]
[Date "1858.11.02"]
[White "Morphy, Paul"]
[Black "Allies"]
[Result "*"]

1. e4 e5 2. Nf3 d6 3. d4 Bg4 4. dxe5 Bxf3 5. Qxf3 dxe5 6. Bc4 Nf6 7. Qb3 Qe7
8. Nc3 c6 9. Bg5 b5 10. Nxb5 cxb5 *
PGN

# A season and more of one club player, for the Player summary pictures: dates, ratings and
# ECO codes, so every part of the summary shows. Made up for the demo.
cat > "$dir/Games/players.pgn" <<'PGN'
[Event "Club league"]
[Site "Demo"]
[Date "2019.03.10"]
[White "Hansen, Anna"]
[Black "Berg, Ola"]
[WhiteElo "1850"]
[BlackElo "1910"]
[ECO "B22"]
[Opening "Sicilian, Alapin"]
[Result "1-0"]

1. e4 c5 2. c3 d5 1-0

[Event "Club league"]
[Site "Demo"]
[Date "2019.11.02"]
[White "Larsen, Mads"]
[Black "Hansen, Anna"]
[WhiteElo "1990"]
[BlackElo "1880"]
[ECO "D02"]
[Opening "Queen's pawn game"]
[Result "1-0"]

1. d4 d5 2. Nf3 Nf6 1-0

[Event "Club league"]
[Site "Demo"]
[Date "2020.06.21"]
[White "Hansen, Anna"]
[Black "Dahl, Eva"]
[WhiteElo "1925"]
[BlackElo "1870"]
[ECO "B22"]
[Opening "Sicilian, Alapin"]
[Result "1/2-1/2"]

1. e4 c5 2. c3 Nf6 1/2-1/2

[Event "Club league"]
[Site "Demo"]
[Date "2021.09.14"]
[White "Moe, Lars"]
[Black "Hansen, Anna"]
[WhiteElo "2050"]
[BlackElo "1960"]
[ECO "C42"]
[Opening "Petrov"]
[Result "0-1"]

1. e4 e5 2. Nf3 Nf6 3. Nc3 0-1

[Event "Club league"]
[Site "Demo"]
[Date "2022.04.03"]
[White "Hansen, Anna"]
[Black "Strand, Per"]
[WhiteElo "2010"]
[BlackElo "2120"]
[ECO "C50"]
[Opening "Italian"]
[Result "1-0"]

1. e4 e5 2. Nf3 Nc6 3. Bc4 1-0

[Event "Club league"]
[Site "Demo"]
[Date "2023.01.28"]
[White "Lie, Ida"]
[Black "Hansen, Anna"]
[WhiteElo "1940"]
[BlackElo "2040"]
[ECO "D02"]
[Opening "Queen's pawn game"]
[Result "0-1"]

1. d4 d5 2. Nf3 e6 0-1

[Event "Club league"]
[Site "Demo"]
[Date "2024.05.19"]
[White "Hansen, Anna"]
[Black "Holm, Siri"]
[WhiteElo "2075"]
[BlackElo "2160"]
[ECO "B22"]
[Opening "Sicilian, Alapin"]
[Result "0-1"]

1. e4 c5 2. c3 e6 0-1

[Event "Club league"]
[Site "Demo"]
[Date "2025.02.08"]
[White "Vik, Jon"]
[Black "Hansen, Anna"]
[WhiteElo "2010"]
[BlackElo "2105"]
[ECO "C42"]
[Opening "Petrov"]
[Result "0-1"]

1. e4 e5 2. Nf3 Nf6 3. Nxe5 0-1
PGN

# A Game with a Comment in German, for Translate in a Library Game (pages.py, model()).
cat > "$dir/Games/comments.pgn" <<'PGN'
[Event "Demo"]
[Site "Demo"]
[Date "2026.01.03"]
[White "Weiß"]
[Black "Schwarz"]
[Result "*"]

1. e4 e5 2. Nf3 Nc6 3. Bb5 {Die Spanische Partie. Weiß greift den Springer an, der den Bauern auf e5 verteidigt.} a6 4. Ba4 Nf6 *
PGN

# Two short 9×9 go Games, two shogi Games (CSA), two xiangqi Games and one draughts Game, made
# up for the demo.
cat > "$dir/Games/go.sgf" <<'SGF'
(;FF[4]GM[1]SZ[9]KM[7.5]PB[Sato, Ken]PW[Lee, Min]DT[2026-09-12]RE[B+3.5];B[ee];W[gc];B[cg];W[gg];B[ge];W[he];B[gf];W[hf];B[fg];W[gh];B[cc];W[eg];B[ef];W[dg];B[dh];W[fh];B[ch];W[hd];B[fc];W[gb];B[fb])
(;FF[4]GM[1]SZ[9]KM[7.5]PB[Lee, Min]PW[Sato, Ken]DT[2026-09-19]RE[W+R];B[ec];W[eg];B[gf];W[cf];B[ce];W[be];B[de];W[bd];B[cc];W[fe];B[ff];W[ef];B[ge];W[fd])
SGF
cat > "$dir/Games/shogi1.csa" <<'CSA'
N+Mori Aki
N-Ueda Jun
PI
+
+7776FU
-3334FU
+2726FU
-8384FU
+2625FU
-8485FU
+6978KI
-4132KI
+2524FU
-2324FU
+2824HI
-0023FU
+2426HI
%TORYO
CSA
cat > "$dir/Games/shogi2.csa" <<'CSA'
N+Ueda Jun
N-Mori Aki
PI
+
+2726FU
-3334FU
+2625FU
-2233KA
+7776FU
-4132KI
%TORYO
CSA
cat > "$dir/Games/xiangqi.pgn" <<'PGN'
[Game "Chinese Chess"]
[Event "Club night"]
[Red "Wang, Lei"]
[Black "Chen, Yu"]
[Result "1-0"]

1. h2e2 h9g7 2. h0g2 i9h9 3. i0h0 b9c7 4. b0c2 c6c5 5. c3c4 b7b3 1-0

[Game "Chinese Chess"]
[Event "Club night"]
[Red "Chen, Yu"]
[Black "Wang, Lei"]
[Result "1/2-1/2"]

1. b2e2 b9c7 2. b0c2 h9g7 3. a0b0 a9b9 1/2-1/2
PGN
cat > "$dir/Games/draughts.pdn" <<'PDN'
[Event "Club night"]
[White "de Vries, Anna"]
[Black "Bakker, Bram"]
[Result "2-0"]

1. 32-28 18-23 2. 37-32 12-18 3. 41-37 7-12 4. 46-41 1-7 2-0
PDN

repo="$(cd "$(dirname "$0")/../.." && pwd)"
lib="$dir/.local/share/bragd/libraries/main.bragd"
(cd "$repo" && cargo run -q --release -p bragd-library --example demo_library -- "$lib" twic "$dir/Games/demo.pgn")
(cd "$repo" && cargo run -q --release -p bragd-library --example demo_library -- "$lib" lichess "$dir/Games/copies.pgn")
(cd "$repo" && cargo run -q --release -p bragd-library --example demo_library -- "$lib" twic "$dir/Games/players.pgn")
(cd "$repo" && cargo run -q --release -p bragd-library --example demo_library -- "$lib" club "$dir/Games/comments.pgn" "$dir/Games/go.sgf" \
  "$dir/Games/shogi1.csa" "$dir/Games/shogi2.csa" "$dir/Games/xiangqi.pgn" "$dir/Games/draughts.pdn" backgammon)

# A stand-in UCI engine for the pictures of Play against an Engine (pages.py sets it as
# Stockfish's program): it answers from one line of the Italian game.
install -m 755 "$repo/docs/screenshots/fake-uci" "$dir/fake-uci"
# A stand-in for KataGo, Fairy-Stockfish (shogi and xiangqi) and GNU Backgammon: legal moves,
# made-up scores (crates/bragd-engines/examples/fake-engine.rs). pages.py sets it as their program.
(cd "$repo" && cargo build -q --release -p bragd-engines --example fake-engine)
install -m 755 "$repo/target/release/examples/fake-engine" "$dir/fake-engine"

echo "demo home: $dir"
