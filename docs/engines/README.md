[← README](../../README.md) · [Docs index](../README.md)

# Engines

An Engine plays a Side in Play, analyses a position in Analyse and reviews a whole Game. Bragd uses an Engine installed on your machine first. Later it can also run one from a sealed container image. These pages cover finding and choosing Engines, analysis, reviews, tablebases, opening books and matches between Engines.

| Page | What it covers |
|---|---|
| [The Engines page](engines.md) | Engines found, adding one by its path, the default per Variant, options |
| [Engine container images](container-images.md) | Sealed images, podman or docker, pins, pruning |
| [Analyse](analyse.md) | Engine lines on any position, clicking into a line |
| [Game review](review.md) | Marks on every move, the evaluation profile |
| [Tablebases](tablebases.md) | Syzygy sets, their folder, the Lichess fallback |
| [Opening books](opening-books.md) | Polyglot books and opening names |
| [Engine matches](matches.md) | Round robins, gauntlets and SPRT matches between Engines, played in the Helper |
| [Background analysis](background-analysis.md) | The Helper's queue of positions for the Position store, and its limits |

## Which Engine plays which Family

| Game family | Variants | Engines Bragd knows | Protocol |
|---|---|---|---|
| Chess | chess, Chess960 | Stockfish, Fairy-Stockfish | UCI |
| Chess | shatranj, chaturanga, King of the Hill, Racing Kings, Seirawan, Capablanca | Fairy-Stockfish | UCI |
| Xiangqi | xiangqi | Fairy-Stockfish, Pikafish, ElephantEye | UCI (UCCI for ElephantEye and other old xiangqi engines) |
| Xiangqi | janggi | Fairy-Stockfish | UCI |
| Shogi | shogi | Fairy-Stockfish, YaneuraOu | USI |
| Go | 9×9, 13×13, 19×19 | KataGo, GNU Go | GTP |
| Draughts | International draughts | Scan | Hub |
| Draughts | English, Brazilian and Russian draughts | none yet | |
| Backgammon | backgammon | GNU Backgammon | its command-line mode |
| Tafl, Ur, Senet | all | none: built-in Bots come later | |

A Family with no Engine shows only Play, Databases and Rules in the sidebar. You can also add any other program that speaks the Family's protocol ([The Engines page](engines.md#how-do-i-add-an-engine-bragd-does-not-know)).

---
[← Previous: How Peers talk](../peers/protocol.md) · [Next: The Engines page →](engines.md)
