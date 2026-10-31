[← README](../../README.md) · [Docs index](../README.md) · [Knowledge](README.md)

# Strip rules

A Strip rule decides which of a Source's Games are kept, for example rating 2000 and up, or slow time controls only. Each rule belongs to a Route, which pairs it with the Library its Games go to ([Sources](sources.md#routes)). It applies to Games as they arrive: what it strips is never stored in that Route's Library. With **Apply** it also applies to what is stored already, in the Source's downloads and in the Route's Library. A Slice filters what you see; a Strip rule removes.

![A Source's options open: schedule, disk cap, largest download, After import, then its Routes, each with its Library and its Strip rule with lowest rating, time controls, results and players](../screenshots/strip-rule-1100.png)
*Databases → Sources → a Source → Options, with a preview.*

## How to reach it

1. In the sidebar, open a Family and choose **Databases**, then **Sources**.
2. Press **Options** on a Source. Each of its Routes has its Strip rule, below its Library.
3. Fill in the parts you want. Every part you fill in must agree for a Game to be kept; a part left empty keeps everything.
4. **Preview** saves the options and counts what would go, from the downloads and each Route's Library, with the space it gives back. Nothing changes.
5. **Apply** saves the options and removes what the rules strip.
6. **Keep everything** empties that Route's rule.

**Prune now** on the *Disk use* tab applies the rule of every Source's Route to the main Library at once ([Disk watch](disk-watch.md)).

| Part | What a Game must have to be kept |
|---|---|
| Lowest rating, Rating of | A rating at least this, for both players, either player, or their average. A missing rating fails |
| Fewest half-moves | At least this many plies |
| Time controls | Bullet, Blitz, Rapid, Classical or Correspondence |
| Results | One of the results ticked |
| Rated Games only | Not casual or unrated by its Event |
| Titled players only | A titled player. Lichess's BOT is not a title |
| From, To | A date in the range, such as `2024.01` to `2024.12.31`. A Game with no known year fails |
| Variants | One of these, by the Source's name or Bragd's id, such as `standard, chess960`. `Standard` and `chess` are one, as are `Chess960` and `chess 960` |
| Players | One of these names, as the Source writes them, apart by commas |

## How the time control is told

Bragd follows Lichess: the base time plus 40 increments. Under 3 minutes is bullet, under 8 blitz, under 25 rapid, and the rest classical. `-` and `1/n` mean correspondence. Without a readable `TimeControl` header, the Event's words decide; FICS "standard" is classical. A Game whose time control cannot be told, such as most over-the-board Games, is kept by a time control rule.

## What it needs

A Source that this version can fetch, and the [Helper](helper.md). Preview and Apply are Helper jobs; their result shows on the Source's line.

## Settings and config.toml

A Strip rule lives in its Route, in the Source's options in the Helper's `state.json`, not in `config.toml`. So do *After import* and what [Disk watch](disk-watch.md) may do by itself for the Source.

## Questions

#### Does Apply delete Games another Source brought?

No. A Game goes from a Library only when each of its Sources has a Route to that Library with a rule, and every one of those rules strips it. A Game another Source brought stays, with all its Sources recorded. **Apply** on one Source looks only at the Games that Source brought, in each of its Routes' Libraries.

#### What happens to a download when a rule is applied?

It is written again with only the Games some Route keeps, into a file beside it. While one Route keeps everything, the download is left as it is. The new file goes beside the old one, and renamed over the old one when complete. A stop or a crash leaves the old file whole. The rewrite keeps its packing; in a zip only plain PGN and PDN members are rewritten.

#### Why is a rewritten `.zst` download larger per Game than the original?

Bragd writes zstd at its one level, about `zstd -1`, which packs less tightly than Lichess's own files.

#### Is Preview exact?

The number of Games is exact. The bytes given back are an estimate: for a file, by the share of game text kept; for the Library, by the share of Games.

#### Can a rule run by itself?

Yes. *After import: Keep only what the Strip rule keeps* shrinks each download to what some Route keeps once it is imported. And Disk watch can apply the rules when a limit is crossed, if you turn that on.

#### Why does the Library not shrink on disk at once?

A Library made before this version is compacted whole after a prune, which needs about as much free space as the Library for a while. Newer Libraries give the space back as they go.

---
[← Previous: Sources](sources.md) · [Next: Disk watch →](disk-watch.md)
