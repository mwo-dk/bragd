[← README](../../README.md) · [Docs index](../README.md) · [Reference](README.md)

# Privacy

Bragd sends nothing unless you ask, with one exception: the update check, which you can turn off. There is no telemetry, no account and no crash report. This page lists every request Bragd can make, when it makes it, and how to stop it. In F1 → Features the update check is the entry `update`.

![Settings at Privacy and updates: the switch Check for a new version with its hint, then the Disk use page](../screenshots/settings-privacy-1100.png)
*Settings → Privacy and updates.*

## Contents

- [What can leave the machine](#what-can-leave-the-machine)
- [The update check](#the-update-check)
- [Links that open your browser](#links-that-open-your-browser)
- [Settings and config.toml](#settings-and-configtoml)
- [Questions](#questions)

## What can leave the machine

| What | When | What is sent | To | How to stop it |
|---|---|---|---|---|
| Update check | At start, then once a day, while on | A request for the newest version, with `bragd/<version>` as the user agent | GitHub, or this copy's channel ([below](#the-update-check)) | *Check for a new version* off (`updates.check`) |
| Source downloads | On the schedule of each Source you turned on, or *Fetch now* | Requests for the files | The Source's address, such as `theweekinchess.com` for TWIC, or your custom URL | Leave the Source off. Every Source starts off ([Sources](../knowledge/sources.md)) |
| Tablebase sizes | When you press **Show size** | A request for one small list of file sizes | `tablebase.lichess.ovh` | Do not press it |
| Tablebase downloads | When you press **Download**, or **Done** in the wizard with Syzygy ticked | Requests for the files | `tablebase.lichess.ovh` | Untick Syzygy in the wizard; do not press Download ([Tablebases](../engines/tablebases.md)) |
| Tablebase lookups | In Analyse, for a chess position of seven pieces or fewer that your tables do not cover | The position, as a FEN | the Lichess tablebase API, `tablebase.lichess.ovh` | *Ask Lichess for tablebase positions* off (`tablebases.online`) |
| Engine images | When you press **Get it** on an Engine's card | A pull of the pinned image | `ghcr.io` | Do not press it, or turn off `engines.containers`. No image is published yet ([Engine container images](../engines/container-images.md)) |
| Pikafish's evaluation network | When you press **Agree and download** under its licence on the Engines page | A request for Pikafish's release archive | `github.com`, Pikafish's release | Do not press it. An installed Pikafish needs none ([Engine container images](../engines/container-images.md#why-does-the-pikafish-image-need-a-download-of-its-own)) |

An Engine Bragd runs from an image has no network at all. An installed Engine is your own program: Bragd gives it no network rules.

## The update check

The check asks for one small file and reads a version number from it. It never downloads or installs an update. When a newer version is out, a notice names it with the command that upgrades your copy.

| Installed from | Asks |
|---|---|
| GitHub release, install script, Homebrew, our Nix flake | `api.github.com`, the latest release of `mwo-dk/bragd` |
| WinGet | The package's folder in `microsoft/winget-pkgs` on GitHub, then GitHub's release |
| Flathub | The app's manifest in `flathub/io.github.mwo_dk.Bragd` on GitHub, then GitHub's release |
| nixpkgs | The package file in `NixOS/nixpkgs` on GitHub, then GitHub's release |

When the channel cannot be asked, GitHub's answer is used, and the notice says the channel may not have the version yet. That is one request a day, two at most. Each waits at most 5 s, and no answer means no news.

The check uses your system's certificate store, so it works behind a proxy your system trusts. So do Source downloads, tablebase downloads and the Lichess tablebase API, and each gives up on a connection that sends nothing for two minutes.

## Links that open your browser

These open a page in your browser. Bragd itself sends nothing:

- *Show me* in F1 → Features opens the feature's page on GitHub.
- *Suggest a better translation* opens a new GitHub issue, with the language, the version and the text to mend filled in. Nothing is sent until you submit it there.
- The install wizard's links to Engine download pages and to online servers.

## Settings and config.toml

| Setting | `config.toml` key | Default | What it does |
|---|---|---|---|
| Check for a new version | `updates.check` | `true` | Ask this copy's channel for the newest version at start and once a day |
| Ask Lichess for tablebase positions | `tablebases.online` | `true` | Send a position your tables do not cover to the Lichess tablebase API |
| Pull an Engine that is not installed | `engines.containers` | `true` | Allow pulling an Engine's image when you ask |

Setting `BRAGD_OFFLINE` in the environment also stops the update check, every tablebase download and lookup, the network download and every Source fetch ([Command line](command-line.md#environment-variables)).

## Questions

#### Does Bragd send my games anywhere?

No. Games, Libraries and Slices stay on your machine. Only a single position goes out, to the Lichess tablebase, and only while that switch is on.

#### Does the update check send anything about me?

It sends an ordinary web request with `bragd/<version>` as its user agent. GitHub sees your address, as with any web request. Nothing else is sent.

#### Can I turn everything off at once?

Turn off *Check for a new version* and *Ask Lichess for tablebase positions*, and leave every Source off. Bragd then makes no request unless you press a download button.

#### What will online play send?

Online play is not available yet. When it comes, signing in to a server is per server, and tokens are kept in your system's keychain only ([Playing online](../online/playing-online.md)).

#### Is the Helper a server others can reach?

No. It listens on a socket in your own data folder, readable by you only. On Windows it listens on a loopback port with a token, which only this machine can reach.

---
[← Previous: Files on disk](files.md) · [Next: Rules →](../rules/README.md)
