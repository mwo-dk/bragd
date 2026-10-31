# Bragd's Koivisto image: Koivisto 9.0 built from its released source (checked by its digest)
# with its evaluation network inside (checked by its digest), linked statically, alone in an
# otherwise empty image. Built for amd64 and arm64 and pushed to ghcr.io/mwo-dk/bragd-koivisto.
# Bragd runs it for chess when no Koivisto is installed (crates/bragd-engines), without network,
# read-only, with no capabilities, as user 65534, talking UCI over stdin and stdout.
#
# The network is default.net from KoivistoNetworks at the commit Koivisto 9.0's networks
# submodule names, so the release plays with the network it was made with. KoivistoNetworks is
# MIT; its licence is in the image as /COPYING.network.
#
# The image holds no shell and no package manager: only /koivisto and the two licences.
# Koivisto is GPL-3.0-or-later; its source is the release named here, unchanged.

FROM docker.io/library/alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS build
ARG TARGETARCH
RUN apk add --no-cache build-base curl
WORKDIR /src
RUN curl -fsSLo koivisto.tar.gz https://github.com/Luecx/Koivisto/archive/refs/tags/v9.0.tar.gz \
 && echo "6528e2607729c18efbb83406884a1a09ae36004c88816dc4dede5bcb0d7d762d  koivisto.tar.gz" | sha256sum -c - \
 && tar xzf koivisto.tar.gz --strip-components=1 && rm koivisto.tar.gz
# Named by EVALFILE, so the makefile does not fetch the submodule with git.
RUN curl -fsSLo /src/koivisto.net https://raw.githubusercontent.com/Luecx/KoivistoNetworks/8cae81230c6bf36733a292ee7633602dfbbe9c49/default.net \
 && echo "cbca457c7e0f1cb9b2ff9a84c2e733479aafed3349cc44eac4fa0dd6af17dab7  /src/koivisto.net" | sha256sum -c - \
 && curl -fsSLo /src/LICENSE.network https://raw.githubusercontent.com/Luecx/KoivistoNetworks/8cae81230c6bf36733a292ee7633602dfbbe9c49/LICENSE \
 && echo "7b139360366161307a97302cc9d1eb983dcf5d7debfe0fc28ff22fdae3f635a8  /src/LICENSE.network" | sha256sum -c -
# Any CPU of its kind: the makefile's own POPCNT build on amd64 (SSE4.2 and POPCNT), plain ARMv8
# on arm64 (NEON is part of it).
# musl gives a new thread 128 KB of stack; the search threads get 8 MB, as glibc gives them.
RUN cd src_files \
 && simd=$([ "$TARGETARCH" = arm64 ] || echo POPCNT=1) \
 && make build CXX="g++ -Wl,-z,stack-size=8388608" $simd DEBUG=0 PGO=0 LTO=1 DETECT=0 STATIC=1 NAMING=0 EVALFILE=/src/koivisto.net EXE=/src/koivisto \
 && strip /src/koivisto \
 && { echo bench | /src/koivisto >bench.txt 2>&1 || { cat bench.txt; exit 1; }; }

FROM scratch
COPY --from=build /src/koivisto /koivisto
COPY --from=build /src/LICENSE.md /COPYING
COPY --from=build /src/LICENSE.network /COPYING.network
LABEL org.opencontainers.image.title="bragd-koivisto" \
      org.opencontainers.image.description="Koivisto 9.0 with its network, static, for Bragd's chess engine when none is installed" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="GPL-3.0-or-later AND MIT"
USER 65534:65534
ENTRYPOINT ["/koivisto"]
