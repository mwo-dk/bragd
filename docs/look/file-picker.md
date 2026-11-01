[← README](../../README.md) · [Docs index](../README.md) · [Look](README.md)

# File picker

Every dialog Bragd shows to open or save a file, or to choose a folder, is your system's own dialog. When Coxswain, a separate app, is installed, its picker can be used instead, and *Settings → Look → File picker* chooses which.

Coxswain's picker shows full names, searches as you type, and looks the same on every system. Bragd offers it when `coxswain-gui` is on your `PATH` and knows `--pick`. Without it, *File picker* has one choice, *The system's dialog*, and says nothing about Coxswain.

## How to reach it

| Where | What to do |
|---|---|
| Settings | *Settings → Look → File picker*: *Automatic*, *Coxswain* or *The system's dialog* when Coxswain is installed; only *The system's dialog* otherwise |
| Databases | **Open…** and **Save as…** on a Foreign database |
| Engines | **Choose…** next to *Path to the engine program* |
| Tablebases | **Choose…** next to the tablebase folder |
| Opening books | **Choose…** next to the book's path |

## Settings and config.toml

| Setting | `config.toml` key | Default | What it does |
|---|---|---|---|
| File picker | `look.file_picker` | `"auto"` | `auto`: Coxswain's picker when `coxswain-gui --help` names `--pick`, else the system's. `coxswain`: Coxswain's, else the system's when it is not found. `system`: always the system's |

With Coxswain installed, *Automatic* reads *Automatic: Coxswain's picker, found on your PATH*.

## Questions

#### How does Bragd talk to Coxswain's picker?

It runs `coxswain-gui --pick` with what the dialog asks for: `--folder`, `--multiple`, `--save NAME`, `--filter "Label:ext,…"`, `--start DIR` and `--title T`. The picker prints each chosen path on a line of its own. When you cancel, it exits with 1, and Bragd does nothing.

#### I installed Coxswain, but Bragd still shows the system's dialog. Why?

Bragd looks for `coxswain-gui` once, when it starts, on your `PATH`, and uses it only if `coxswain-gui --help` mentions `--pick`. An older Coxswain does not have it: update Coxswain, then restart Bragd.

#### Does the picker see more of my files than Bragd does?

No. It only hands Bragd the paths you chose. Bragd opens nothing until you have chosen.

---
[← Previous: Languages](languages.md) · [Next: Online →](../online/README.md)
