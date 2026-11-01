[← README](../../README.md) · [Docs index](../README.md) · [Reference](README.md)

# Files on disk

Bragd keeps its files in three folders of your own: config, data and cache. Each holds a `bragd` folder. Nothing is written beside the program, and nothing outside these folders unless you choose a folder in Settings.

## Contents

- [The three folders](#the-three-folders)
- [What is in them](#what-is-in-them)
- [The Helper's registration](#the-helpers-registration)
- [Questions](#questions)

## The three folders

| Folder | Linux and the BSDs | macOS | Windows |
|---|---|---|---|
| Config | `$XDG_CONFIG_HOME/bragd`, else `~/.config/bragd` | `~/Library/Application Support/bragd` | `%APPDATA%\bragd` |
| Data | `$XDG_DATA_HOME/bragd`, else `~/.local/share/bragd` | `~/Library/Application Support/bragd` | `%APPDATA%\bragd` |
| Cache | `$XDG_CACHE_HOME/bragd`, else `~/.cache/bragd` | `~/Library/Caches/bragd` | `%LOCALAPPDATA%\bragd` |

In the Flatpak, the XDG folders are the app's own: `~/.var/app/io.github.mwo_dk.Bragd/config`, `/data` and `/cache`.

## What is in them

| Path | What it is |
|---|---|
| config/`config.toml` | Your settings ([config.toml](config-toml.md)) |
| config/`engine-options.toml` | Each Engine's options, as you set them on the Engines page |
| data/`state.json` | What Bragd remembers that is not a setting: the wizard seen, the last version whose cards you saw, the last update check, notices closed, hints shown, the Club files you opened |
| data/`libraries/` | Your Libraries, such as `main.bragd`, with their `-wal` and `-shm` files. Moved by `knowledge.library` |
| data/`clubs/` | The Clubs you made, one `<name>.bragd-club` file each, with a `.bragd-club.lock` beside a Club while it is open ([Clubs](../clubs/clubs.md)) |
| data/`positions.sqlite` | The [Position store](../knowledge/position-store.md): Engine Evaluations and Annotations by position, with its `-wal` and `-shm` files |
| data/`translations.sqlite` | Comments translated by the [language model](../knowledge/language-model.md), by their original text and language. Made by the first translation |
| data/`rating-lists.sqlite` | The players of every rating list fetched ([Rating lists](rating-lists.md)) |
| data/`tablebases/` | Syzygy tables. Moved by `tablebases.path` |
| data/`networks/pikafish/pikafish.nnue` | Pikafish's evaluation network, for its image, downloaded once you agree to its licence |
| data/`helper/` | The Helper's folder |
| data/`helper/helper.lock` | Held while a Helper runs, so only one runs |
| data/`helper/helper.sock` | Where windows reach the Helper (Unix). On Windows it listens on a loopback port instead |
| data/`helper/helper.addr` | The Helper's address and a token, readable by you only |
| data/`helper/helper.log` | What the Helper did, with times in UTC |
| data/`helper/peers/key.pk8` | This machine's key for [Peers](../peers/protocol.md#keys), readable by you only. Made when Peers are first used |
| data/`helper/peers/peers.json` | The machines paired with this one: each one's key, name and last address |
| data/`helper/state.json` | Sources, their options and Routes with their Strip rules, what each fetch remembered, and the jobs |
| data/`helper/sources/<source>/` | Each Source's downloads, kept as they came, one folder per Source id (`custom:club.a` is `custom_3aclub_2ea`) |

A file a Source downloads stays packed. Zips and packed PGN are read as a stream and never unpacked to disk.

## The Helper's registration

Turning *Start with my session* on writes one of these, as `bragd-helper --service on` does; turning it off removes it ([Command line](command-line.md#bragd-helper)):

| System | What | Where |
|---|---|---|
| Linux | A systemd user unit | config/`systemd/user/bragd-helper.service` (the XDG config folder, not Bragd's) |
| The BSDs | An XDG autostart entry | config/`autostart/bragd-helper.desktop` |
| Flatpak | An XDG autostart entry that runs the Flatpak | `~/.config/autostart/bragd-helper.desktop` |
| macOS | A LaunchAgent | `~/Library/LaunchAgents/dk.mwo.bragd.helper.plist` |
| Windows | A Run entry | the registry, under `HKCU\…\Run`, named `bragd-helper` |

## Questions

#### How do I start again from scratch?

Close every window and wait for the Helper to leave, or run `bragd-helper --service off`. Then delete the `bragd` folders in the config, data and cache folders. Your Foreign databases are elsewhere and are not touched.

#### Can my Libraries live on another disk?

Yes. Set *Library folder* in *Settings → Knowledge*. Disk watch then also checks free space on that disk.

#### What if state.json is damaged?

Bragd moves it aside as `state.json.bad` and starts with defaults. A file it cannot read at all is never written over.

#### Does Bragd write next to my PGN files?

No, unless you choose *Save as…* there. A Foreign database is read in place and only changed when you save to it ([Foreign databases](../knowledge/foreign-databases.md)).

#### Why does uninstalling leave files behind?

Installers remove the program, not your settings and Libraries. Delete the folders above to remove them too.

---
[← Previous: Command line](command-line.md) · [Next: Privacy →](privacy.md)
