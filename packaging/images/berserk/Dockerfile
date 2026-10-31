# Bragd's Berserk image: Berserk 14 built from its released source (checked by its digest) with
# its evaluation network inside (checked by its digest), linked statically, alone in an
# otherwise empty image. Built for amd64 and arm64 and pushed to ghcr.io/mwo-dk/bragd-berserk.
# Bragd runs it for chess when no Berserk is installed (crates/bragd-engines), without network,
# read-only, with no capabilities, as user 65534, talking UCI over stdin and stdout.
#
# The network is the one Berserk 14's makefile names, from Berserk's own networks release; its
# name carries the start of its SHA-256, which the makefile checks, and here the whole digest
# is checked. It has no licence of its own: Berserk's releases ship it inside the GPL-3.0
# program, as this image does.
#
# The image holds no shell and no package manager: only /berserk and its licence. Berserk is
# GPL-3.0-or-later; its source is the release named here, unchanged.

FROM docker.io/library/alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS build
ARG TARGETARCH
RUN apk add --no-cache build-base curl
WORKDIR /src
RUN curl -fsSLo berserk.tar.gz https://github.com/jhonnold/berserk/archive/refs/tags/14.tar.gz \
 && echo "3031835f691cf6f42e31c4f385563ac44649ca1dc44e0b86459de8ebef9e1b2d  berserk.tar.gz" | sha256sum -c - \
 && tar xzf berserk.tar.gz --strip-components=1 && rm berserk.tar.gz
# In src/ before the build, so the makefile fetches nothing.
RUN curl -fsSLo src/berserk-9b84c340af7e.nn https://github.com/jhonnold/berserk-networks/releases/download/networks/berserk-9b84c340af7e.nn \
 && echo "9b84c340af7e45f6e07f0046235ccb327f4ae0840c8ee2c4b97b99121e5c5084  src/berserk-9b84c340af7e.nn" | sha256sum -c -
# Any CPU of its kind: the makefile's own SSE4.1 and POPCNT flags on amd64, NEON on arm64.
# Berserk's NEON code tests __ARM_NEON__, which only Apple's compiler sets, so it is set here.
# musl gives a new thread 128 KB of stack; the search threads get 8 MB, as glibc gives them.
RUN cd src \
 && simd=$([ "$TARGETARCH" = arm64 ] && echo -D__ARM_NEON__ || echo '$(MSSE41)') \
 && make all CC=gcc CFLAGS="\$(FLAGS) $simd -static -Wl,-z,stack-size=8388608" \
 && strip berserk \
 && { ./berserk bench 8 >bench.txt 2>&1 || { cat bench.txt; exit 1; }; }

FROM scratch
COPY --from=build /src/src/berserk /berserk
COPY --from=build /src/LICENSE /COPYING
LABEL org.opencontainers.image.title="bragd-berserk" \
      org.opencontainers.image.description="Berserk 14 with its network, static, for Bragd's chess engine when none is installed" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="GPL-3.0-or-later"
USER 65534:65534
ENTRYPOINT ["/berserk"]
