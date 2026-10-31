# Bragd's RubiChess image: RubiChess 20240817 built from its released source (checked by its
# digest) with its evaluation network inside (checked by its digest), linked statically, alone
# in an otherwise empty image. Built for amd64 and arm64 and pushed to
# ghcr.io/mwo-dk/bragd-rubichess. Bragd runs it for chess when no RubiChess is installed
# (crates/bragd-engines), without network, read-only, with no capabilities, as user 65534,
# talking UCI over stdin and stdout.
#
# The network is the one the release names (NNUEDEFAULT in RubiChess.h), from its author's NN
# repository at a fixed commit; its name carries the start of its SHA-256, which the makefile
# checks, and here the whole digest is checked. It has no licence of its own: RubiChess's
# releases ship it inside the GPL-3.0 program, as this image does.
#
# The image holds no shell and no package manager: only /rubichess and its licence. RubiChess
# is GPL-3.0-or-later; its source is the release named here, unchanged.

FROM docker.io/library/alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS build
ARG TARGETARCH
RUN apk add --no-cache build-base curl
WORKDIR /src
RUN curl -fsSLo rubichess.tar.gz https://github.com/Matthies/RubiChess/archive/refs/tags/20240817.tar.gz \
 && echo "8e90f0477980960ffe4bc46a955b5423bf00416c385d4a7f4befa79e363fb972  rubichess.tar.gz" | sha256sum -c - \
 && tar xzf rubichess.tar.gz --strip-components=1 && rm rubichess.tar.gz
# In src/ before the build, so the makefile fetches nothing.
RUN curl -fsSLo src/nn-bc638d5ec9-20240730.nnue https://raw.githubusercontent.com/Matthies/NN/bc471a62eb8c0f21b71f084a2ee7865e829f3f4b/nn-bc638d5ec9-20240730.nnue \
 && echo "bc638d5ec9f15e5de7a4d1c32f7208e4e7cf889e566d225309915ae38a6542b5  src/nn-bc638d5ec9-20240730.nnue" | sha256sum -c -
# Any CPU of its kind: the makefile's x86-64-modern (SSSE3 and POPCNT) on amd64, armv8 (NEON) on
# arm64. On arm64 the makefile adds -mcpu=native when the machine is aarch64, so it is told the
# machine is not.
# musl gives a new thread 128 KB of stack; the search threads get 8 MB, as glibc gives them.
RUN cd src \
 && arch=$([ "$TARGETARCH" = arm64 ] && echo "armv8 UNAME_M=armv8" || echo x86-64-modern) \
 && make build ARCH=$arch COMP=gcc EVALFILE=default EXTRALDFLAGS="-static -Wl,-z,stack-size=8388608" \
 && strip RubiChess \
 && { ./RubiChess -bench -depth 8 >bench.txt 2>&1 || { cat bench.txt; exit 1; }; } \
 && mv RubiChess rubichess

FROM scratch
COPY --from=build /src/src/rubichess /rubichess
COPY --from=build /src/copying /COPYING
LABEL org.opencontainers.image.title="bragd-rubichess" \
      org.opencontainers.image.description="RubiChess 20240817 with its network, static, for Bragd's chess engine when none is installed" \
      org.opencontainers.image.version="20240817" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="GPL-3.0-or-later"
USER 65534:65534
ENTRYPOINT ["/rubichess"]
