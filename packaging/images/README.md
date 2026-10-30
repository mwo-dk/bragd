# Engine images

Each folder here, `packaging/images/<engine>`, is one of Bragd's sealed engine images.
`.github/workflows/images.yml` builds it for amd64 and arm64 and pushes it to
`ghcr.io/mwo-dk/bragd-<engine>:latest`, from master, and again every Saturday.

A folder holds:

| File | What |
|---|---|
| `Dockerfile` | The engine built from its released source, checked by its digest, alone in an otherwise empty image: no shell, no package manager. Every `FROM` is pinned as `name:tag@sha256:…` (`tools/images-pin.py` moves the pins on). |
| `check.sh` (optional, executable) | Given the image's name, talks the engine's protocol to it (UCI, UCCI, USI, GTP or Hub) the way Bragd runs it: `--network=none --read-only --tmpfs /tmp --cap-drop=ALL --security-opt no-new-privileges`. Fails when the engine does not answer. |

The image's licence, and the source it was built from, go into `org.opencontainers.image.*` labels
and into `tools/bom/README.md`.
