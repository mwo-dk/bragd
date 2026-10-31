[← README](../../README.md) · [Docs index](../README.md) · [Knowledge](README.md)

# Sources

A Source is where Games come from, such as The Week in Chess. When a Source is on, the [Helper](helper.md) fetches it on its schedule and imports its Games into `main.bragd`. Every Source is off until you turn it on, and its size per fetch is shown before it downloads anything. You can also add any address of a game file as a custom URL Source.

![The Sources tab for chess: The Week in Chess with its size, last fetch, Options and Fetch now, the big Sources greyed out with their notice, and Add a custom URL at the bottom](../screenshots/databases-sources-1100.png)
*Databases → Sources.*

## How to reach it

1. In the sidebar, open a Family and choose **Databases**, then **Sources**. The tab lists that Family's Sources and every custom URL one.
2. Tick a Source to turn it on. **Fetch now** fetches it at once.
3. **Options** opens its schedule, the metered rule, its disk cap, what becomes of its downloads and its [Strip rule](strip-rules.md). Press **Save**.
4. Under *Add a custom URL*, type a name and the address, and press **Add**.

The install wizard's Knowledge step offers TWIC too, ticked ([The install wizard](../getting-started/first-start.md)).

## The Sources

| Id | Source | Family | About a fetch | Default schedule | Fetched in this version |
|---|---|---|---|---|---|
| `twic` | The Week in Chess | chess | 1.5 MB | Weekly | Yes |
| `custom:<name>` | A custom URL | any | unknown | Weekly | Yes |
| `lichess-db` | Lichess database | chess | 30 GB | Monthly | Not yet |
| `fics` | FICS games database | chess | 300 MB | Monthly | Not yet |
| `pgnmentor` | PGN Mentor | chess | 200 MB | Monthly | Not yet |
| `lumbras` | LumbrasGigaBase | chess | 4 GB | Monthly | Not yet |
| `ogs` | OGS archive | go | 1 GB | Monthly | Not yet |
| `kgs` | KGS archive | go | 100 MB | Monthly | Not yet |
| `pychess` | pychess exports | chess | 500 MB | Monthly | Not yet |
| `lishogi` | Lishogi exports | shogi | 500 MB | Monthly | Not yet |

The Sources marked *Not yet* are listed so you know they are coming. They are greyed out, and a notice says this version cannot fetch them.

**TWIC.** The first fetch takes only the newest issue. Each later fetch takes the new issues in order, at most 26 at a time. The zips are imported as they are, without unpacking.

**Custom URL.** The address must start with `http://` or `https://` and name a PGN, SGF, KIF, CSA or PDN file, or a zip of them. The whole file is downloaded again on each fetch, since it may have grown.

## Options

| Option | Choices | What it does |
|---|---|---|
| Fetch | Daily, Weekly, Monthly, Only when I ask | How often the Helper fetches it. A month is 30 days |
| Only on a connection that is not metered | on, off | Skips a fetch on a metered connection |
| Keep at most | a size such as `500 MB`, or empty | The oldest downloads go first when the Source's downloads pass this |
| Largest download | a size such as `5 GB`, or empty for `2 GB` | The most one download may take without asking. A bigger one waits for your yes |
| After import | Keep the download, Keep only what the Strip rule keeps, Delete the download | What becomes of a download once imported |
| Address | | A custom URL Source's address |

## What it needs

A network connection when it fetches, and the Helper. A failed fetch is not tried again for an hour. A download cut short goes on from where it stopped next time.

## Settings and config.toml

A Source's options and what each fetch remembered live in the Helper's `state.json`, not in `config.toml`. Two keys in `config.toml` apply:

| Setting | `config.toml` key | Default | What it does |
|---|---|---|---|
| Library folder | `knowledge.library` | empty: `<data>/bragd/libraries` | Fetched Games go into `main.bragd` there |
| Source priority | `knowledge.source_priority` | `["twic", "lichess"]` | Which Source wins a header conflict between [Duplicates](duplicates.md) |

## Questions

#### Does Bragd download anything before I turn a Source on?

No. Every Source starts off. Nothing is fetched until you tick it, or tick it in the install wizard and press Done.

#### Why is a Source greyed out?

This version lists it so you know it is coming, but cannot fetch it yet. Its notice says so.

#### What does "metered" mean here?

A connection NetworkManager marks as metered, such as a phone hotspot. On systems without NetworkManager every connection counts as not metered, so this option never blocks a fetch there.

#### Where do the downloads go?

Into the Helper's folder, `<data>/bragd/helper/sources/<source>/`, one folder per Source, named by its id. Letters, digits and `-` stay as they are, and any other character is written `_` and its hex code, so `custom:club.a` is `custom_3aclub_2ea`. Two Sources never share a folder. The downloads are kept unless *After import* says otherwise or *Keep at most* is passed.

#### Why does a Source ask before a big download?

Each download may take at most 2 GB, or what you set as *Largest download*. When the server says a file is bigger, nothing is downloaded. The Source's line then says how big it is, with **Download it anyway**, and a notice says so. Your yes covers that one download, and the limit applies again after it. When the server does not say how big a file is, the download stops as it passes the limit. Raise *Largest download* to take it.

#### Why was a Source not fetched on time?

It may be paused because the disk is over your limits ([Disk watch](disk-watch.md)), the machine may be on battery, the connection may be metered, or the last try failed less than an hour ago. Its line shows the last error.

#### Does a fetch need a window open?

No, when the Helper is running. It stays ten minutes after the last window closes, or all the time with *Start with my session* ([The Helper](helper.md)).

#### Can I remove a custom URL Source?

Not from this version. Turn it off by unticking it, and it is never fetched.

---
[← Previous: Export to Parquet](parquet-export.md) · [Next: Strip rules →](strip-rules.md)
