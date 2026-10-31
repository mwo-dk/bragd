# Bragd's Scan image: Scan 3.1 built from its released source (checked by its digest), linked
# statically, alone in an otherwise empty image with its evaluation and opening book for
# international draughts. Built for amd64 and arm64 and pushed to ghcr.io/mwo-dk/bragd-scan.
# Bragd runs it for draughts when no Scan is installed (crates/bragd-engines), without network,
# read-only, with no capabilities, as user 65534, talking Hub over stdin and stdout. Its
# entrypoint starts Hub.
#
# Scan reads scan.ini and data/ from its working folder, /scan; the archive's files are
# readable by their owner only, so they are installed readable by all. No endgame tables (bb-size=0):
# the 2–6 piece tables are 706 MiB of their own.
#
# The image holds no shell and no package manager: only /scan. Scan is GPL-3.0; its source is
# the release named here, unchanged.

FROM docker.io/library/alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS build
ARG TARGETARCH
RUN apk add --no-cache build-base curl unzip
WORKDIR /src
RUN curl -fsSLo scan.zip https://hjetten.home.xs4all.nl/scan/scan_31.zip \
 && echo "7579cc16cb3650a6bc5e98035a4052fe7941959050f72e4c4b42fc6fde02ece4  scan.zip" | sha256sum -c - \
 && unzip -q scan.zip && rm scan.zip
# POPCNT on amd64; arm64 counts bits without it.
RUN cd scan_31/src \
 && popcnt=$([ "$TARGETARCH" = arm64 ] || echo -mpopcnt) \
 && make -j"$(nproc)" CXXFLAGS="-pthread -std=c++14 -fno-rtti -O2 $popcnt -flto -DNDEBUG" LDFLAGS="-pthread -O2 -flto -static" \
 && strip scan \
 && install -D -m 0755 scan /out/scan \
 && install -m 0644 ../scan.ini ../license.txt /out/ \
 && install -D -m 0644 -t /out/data ../data/eval ../data/book \
 && sed -i 's/^bb-size *=.*/bb-size = 0/; s/^dxp-server *=.*/dxp-server = false/' /out/scan.ini \
 && cd /out && printf 'hub\ninit\npos pos=Wbbbbbbbbbbbbbbbbbbbbeeeeeeeeeewwwwwwwwwwwwwwwwwwww\nlevel depth=4\ngo think\nquit\n' | ./scan hub | grep -q '^done move='

FROM scratch
COPY --from=build /out /scan
WORKDIR /scan
LABEL org.opencontainers.image.title="bragd-scan" \
      org.opencontainers.image.description="Scan 3.1, static, for Bragd's draughts engine when none is installed" \
      org.opencontainers.image.version="3.1" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="GPL-3.0-only"
USER 65534:65534
ENTRYPOINT ["/scan/scan", "hub"]
