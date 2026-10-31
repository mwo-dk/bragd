[← README](../../README.md) · [Docs index](../README.md) · [Reference](README.md)

# Command line

Bragd is two programs. `bragd-gui` is the app with its window. `bragd-helper` is the [Helper](../knowledge/helper.md), the background process every window shares. Both are installed side by side. You rarely start either by hand: the menu entry starts the app, and the app starts the Helper.

## bragd-gui

```text
bragd-gui [--version | -V]
```

| Flag | What it does |
|---|---|
| none | Opens a window |
| `--version`, `-V` | Prints `bragd-gui` and its version, then ends |

## bragd-helper

```text
usage: bragd-helper [--stay | --service [on|off]]
```

| Flag | What it does |
|---|---|
| none | Runs the Helper. It leaves ten minutes after the last window closes and no job runs |
| `--stay` | Runs the Helper and keeps it running when no window is open. The session starts it this way once `--service on` has registered it |
| `--service` | Prints `on` when the Helper is registered to start with your session, `off` when not. The exit code says it too: 0 for on, 3 for off |
| `--service on` | Registers the Helper to start with your session, and starts it |
| `--service off` | Takes the registration away. A Helper the system started stops too |

The registration is a systemd user unit on Linux, a LaunchAgent on macOS, a Run entry in the registry on Windows, and an XDG autostart entry on the BSDs and in the Flatpak ([Files on disk](files.md#the-helpers-registration)).

| Exit code | Means |
|---|---|
| 0 | Done |
| 1 | It failed; the reason is printed after `bragd-helper:` |
| 2 | An unknown flag; the usage line is printed |
| 3 | `--service` alone: the Helper is not registered |

On Windows, `bragd-helper` is a windowed program, so neither it nor the `reg` commands it runs open a console window. It has no console to print to either: read the exit code (`$LASTEXITCODE` in PowerShell, `%ERRORLEVEL%` in `cmd`) or `helper.log` in the Helper's folder, where every `--service` run writes what it did.

## Environment variables

| Variable | Read by | What it does |
|---|---|---|
| `BRAGD_OFFLINE` | `bragd-gui`, `bragd-helper` | Set to anything: nothing is asked online. No update check, no tablebase download or lookup, no engine network download, no Source fetch |
| `BRAGD_DEV_IMAGES` | `bragd-gui` | `1`: also run the engine images `tools/dev/images.sh` built on this machine (`localhost/bragd-<engine>:dev`), which are not pinned ([Engine container images](../engines/container-images.md#images-built-on-your-own-machine)). Without it only pinned images run |
| `BRAGD_WINDOW_SIZE` | `bragd-gui` | `WxH`, such as `1100x760`: the window's size at start. Used for screenshots |

Other `BRAGD_` variables in the code are read only by tests.

## Questions

#### Can two Helpers run at once?

No. The Helper holds a lock in its folder, so a second one does not start. A newer window makes an older Helper leave, and starts its own.

#### How do I stop the Helper?

Close every window: it leaves ten minutes later, once no job runs. A Helper started with `--stay` keeps running; `bragd-helper --service off` stops it.

#### Is `--service on` the same as the switch in Settings?

Yes. Turning *Start with my session* on in *Settings → Knowledge* or in the wizard registers the Helper as `--service on` does. Turning it off is `--service off`.

#### Does `BRAGD_OFFLINE` stop the Helper fetching Sources?

Yes. A Helper started with `BRAGD_OFFLINE` set queues no Source on its schedule, and *Fetch now* fails with an error naming `BRAGD_OFFLINE`. A window passes the variable on to the Helper it starts. A Helper already running keeps the environment it started with.

#### How do I see which version I have?

`bragd-gui --version`, or *Settings → Overview*, which names the version on its first line.

---
[← Previous: Every setting](settings.md) · [Next: Files on disk →](files.md)
