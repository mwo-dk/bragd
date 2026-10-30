# Bragd's Pikafish image: Pikafish 2026-09-06 built from its released source (checked by its
# digest) with its evaluation network inside (checked by its digest; placed before the build, so
# its own scripts/net.sh fetches nothing), linked statically, alone in an otherwise empty image.
# Built for amd64 and arm64 and pushed to ghcr.io/mwo-dk/bragd-pikafish. Bragd runs it for
# xiangqi when no Pikafish is installed (crates/bragd-engines), without network, read-only, with
# no capabilities, as user 65534, talking UCI over stdin and stdout.
#
# The network comes from Pikafish's `master-net` release, which moves: when it does, the digest
# check below fails the build, and the new digest is set here on purpose.
#
# The image holds no shell and no package manager: only /pikafish. Pikafish is GPL-3.0; its
# source is the release named here, unchanged.

FROM docker.io/library/alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS build
ARG TARGETARCH
RUN apk add --no-cache build-base curl
WORKDIR /src
RUN curl -fsSLo pf.tar.gz https://github.com/official-pikafish/Pikafish/archive/refs/tags/Pikafish-2026-09-06.tar.gz \
 && echo "4d905f5bb9d96632900aa88505d816e2d5997919e1375077a591cfeb0e6c7a10  pf.tar.gz" | sha256sum -c - \
 && tar xzf pf.tar.gz --strip-components=1 && rm pf.tar.gz
RUN curl -fsSLo src/pikafish.nnue https://github.com/official-pikafish/Networks/releases/download/master-net/pikafish.nnue \
 && echo "810ef8ecb0d0697755deac0a1d956296d762132f63b74fc4f8658f2ec5203f5b  src/pikafish.nnue" | sha256sum -c -
# Any CPU of its kind: SSE4.1 and POPCNT on amd64, plain ARMv8 on arm64.
RUN cd src \
 && arch=$([ "$TARGETARCH" = arm64 ] && echo armv8 || echo x86-64-sse41-popcnt) \
 && make -j"$(nproc)" build ARCH="$arch" COMP=gcc EXTRALDFLAGS=-static EXTRACXXFLAGS=-U_FORTIFY_SOURCE \
 && strip pikafish \
 && ./pikafish bench 16 1 6 >/dev/null 2>&1

FROM scratch
COPY --from=build /src/src/pikafish /pikafish
COPY --from=build /src/Copying.txt /COPYING
LABEL org.opencontainers.image.title="bragd-pikafish" \
      org.opencontainers.image.description="Pikafish 2026-09-06, static, for Bragd's xiangqi engine when none is installed" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="GPL-3.0-only"
USER 65534:65534
ENTRYPOINT ["/pikafish"]
