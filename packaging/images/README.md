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

After a master build, the same workflow deletes old untagged versions on ghcr.io with
`tools/ghcr-prune.py`. It keeps every tagged version, every digest pinned in
`crates/bragd-engines/src/known.rs` on master and in the newest 3 release tags, the newest 5
untagged images, and the per-platform manifests of each image it keeps: GHCR lists those as
untagged versions of their own. `python3 tools/ghcr-prune.py --self-test` checks the plan against
`tools/fixtures/ghcr-prune/`.
