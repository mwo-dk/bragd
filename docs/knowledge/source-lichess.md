[← README](../../README.md) · [Docs index](../README.md) · [Knowledge](README.md)

# The Lichess database

The Lichess database is every rated Game played on [lichess.org](https://lichess.org), one file a month for each Variant, from [database.lichess.org](https://database.lichess.org/). Bragd fetches the months you choose, keeps each file packed as it came (`.pgn.zst`), and imports it through the Source's Routes as it reads it. Nothing is unpacked to disk.

A month of standard chess is about 30 GB packed and holds about 100 million Games. Read the [questions](#questions) on disk space before you turn it on.

<!-- screenshot: sources-lichess-1100.png: Databases → Sources with the Lichess database's Options open: What to fetch with Chess ticked and Chess960, King of the hill and Racing kings not, and First month empty -->

## How to reach it

1. In the sidebar, open **Chess**, then **Databases**, then **Sources**.
2. Press **Options** on *Lichess database*.
3. Under **What to fetch**, tick the Variants you want. Chess is ticked until you choose.
4. To start from an older month, type it in **First month**, such as `2024-01`. Leave it empty to take only the newest month.
5. Press **Save**, then tick *Lichess database* to turn it on. **Fetch now** starts at once.

The install wizard offers it too, with a warning about its size ([The install wizard](../getting-started/first-start.md)).

## What a fetch does

| Step | What happens |
|---|---|
| List | Bragd reads each chosen Variant's `list.txt` to see which months are out |
| Choose months | The first fetch takes the newest month, or every month from *First month*. Later fetches take each month after the last one fetched, at most 3 at a time |
| Ask | A file bigger than *Largest download* (2 GB unless you change it) is not downloaded. The Source's line shows its size with **Download it anyway** |
| Download | Into `<data>/bragd/helper/sources/lichess-db/`. A download cut short goes on from where it stopped |
| Import | Game by game through the Routes and their Strip rules, straight from the packed file |

Only the Variants Bragd plays are offered: Chess, Chess960, King of the hill and Racing kings. A Variant you turned off in Settings is not shown. A month counts as fetched only when every chosen Variant has it.

## Options

The Source's other options (schedule, metered connections, *Keep at most*, *Largest download*, *After import* and Routes) are those of every Source ([Sources](sources.md#options)).

| Option | What it does |
|---|---|
| What to fetch | The Variants whose files are fetched |
| First month | The month the first fetch starts from, as `yyyy-mm`. Empty: only the newest |

## Questions

#### How much disk does it take?

A standard month is about 30 GB packed, and more each year. Chess960, King of the hill and Racing kings are each well under 1 GB a month. The Library grows too, by far less than the file when a Strip rule keeps only some Games. To keep less: set *After import* to *Keep only what the Strip rule keeps* or *Delete the download*, give the Source a *Keep at most*, and see [Disk watch](disk-watch.md).

#### Why did nothing download?

The file is bigger than *Largest download*. Its line says how big, with **Download it anyway**. Your yes covers that one download. Raise *Largest download* to take every month without asking.

#### Can I change First month after a fetch?

It counts only until the first fetch. After that each fetch goes on from the last month fetched. To take older months later, add them as a [Foreign database](foreign-databases.md) or import the files by hand.

#### Where does it come from, and on what terms?

From `https://database.lichess.org/<variant>/`, released by Lichess under CC0, free to use for anything.

---
[← Previous: Sources](sources.md) · [Next: The FICS games database →](source-fics.md)
