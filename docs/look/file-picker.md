[← README](../../README.md) · [Docs index](../README.md) · [Look](README.md)

# File picker

Every dialog Bragd shows to open or save a file, or to choose a folder, is one of two: Coxswain's picker or your system's own dialog. *Settings → Look → File picker* chooses which.

Coxswain's picker shows full names, searches as you type, and looks the same on every system. It comes with Coxswain, a separate app: Bragd uses it when `coxswain-gui` is on your `PATH` and knows `--pick`.

## How to reach it

| Where | What to do |
|---|---|
| Settings | *Settings → Look → File picker*: *Automatic*, *Coxswain* or *The system's dialog* |
| Databases | **Open…** and **Save as…** on a Foreign database |
| Engines | **Choose…** next to *Path to the engine program* |
| Tablebases | **Choose…** next to the tablebase folder |
| Opening books | **Choose…** next to the book's path |

## Settings and config.toml

| Setting | `config.toml` key | Default | What it does |
|---|---|---|---|
| File picker | `look.file_picker` | `"auto"` | `auto`: Coxswain's picker when `coxswain-gui --help` names `--pick`, else the system's. `coxswain`: Coxswain's, else the system's when it is not found. `system`: always the system's |

*Automatic* says which one it found: *Automatic: Coxswain's picker, found on your PATH*, or *Automatic: the system's dialog, as Coxswain is not found*.

## Questions

#### How does Bragd talk to Coxswain's picker?

It runs `coxswain-gui --pick` with what the dialog asks for: `--folder`, `--multiple`, `--save NAME`, `--filter "Label:ext,…"`, `--start DIR` and `--title T`. The picker prints each chosen path on a line of its own. When you cancel, it exits with 1, and Bragd does nothing.

#### I installed Coxswain, but Bragd still shows the system's dialog. Why?

Bragd looks for `coxswain-gui` once, when it starts, on your `PATH`, and uses it only if `coxswain-gui --help` mentions `--pick`. An older Coxswain does not have it: update Coxswain, then restart Bragd.

#### Does the picker see more of my files than Bragd does?

No. It only hands Bragd the paths you chose. Bragd opens nothing until you have chosen.

---
[← Previous: Languages](languages.md) · [Next: Online →](../online/README.md)
