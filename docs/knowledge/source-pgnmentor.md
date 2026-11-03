[← README](../../README.md) · [Docs index](../README.md) · [Knowledge](README.md)

# PGN Mentor

PGN Mentor collects chess Games by player, by opening and by event, from [pgnmentor.com](https://www.pgnmentor.com/files.html): a zip for each player and opening, and a PGN file for each event. Bragd fetches the files of the sections you choose and imports them through the Source's Routes. A Game found in a player's file and an event's file is stored once ([Duplicates](duplicates.md)).

<!-- screenshot: sources-pgnmentor-1100.png: Databases → Sources with PGN Mentor's Options open: What to fetch with Players, Openings and Events ticked -->

## How to reach it

1. In the sidebar, open **Chess**, then **Databases**, then **Sources**.
2. Press **Options** on *PGN Mentor*.
3. Under **What to fetch**, tick **Players**, **Openings** or **Events**. All three are ticked until you choose.
4. Press **Save**, then tick the Source to turn it on.

## What a fetch does

Each fetch reads the files page and downloads the files of the chosen sections that it has not fetched before. The first fetch takes about 1,500 files, about 200 MB. Later fetches take only files that are new on the page. They go into `<data>/bragd/helper/sources/pgnmentor/`, named by section, such as `players-Adams.zip`.

## Questions

#### Does a fetch take a player's file again when it has new Games?

No. A file is fetched once, by its name, even after its download is deleted. PGN Mentor seldom adds to a file. The newest Games of top players come from [The Week in Chess](sources.md#the-sources) every week.

#### Can I keep only some players?

Fetch the section, then give the Route a [Strip rule](strip-rules.md) with the players you want.

---
[← Previous: The FICS games database](source-fics.md) · [Next: Strip rules →](strip-rules.md)
