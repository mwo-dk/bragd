# Bragd's Fairy-Stockfish image: Fairy-Stockfish 14.0.1 (tag fairy_sf_14_0_1_xq) built from its
# source (checked by its digest) with large boards, so capablanca, xiangqi and janggi play too,
# linked statically, alone in an otherwise empty image. Built for amd64 and arm64 and pushed to
# ghcr.io/mwo-dk/bragd-fairy-stockfish. Bragd runs it for the chess Variants, xiangqi, janggi
# and shogi when no Fairy-Stockfish is installed (crates/bragd-engines), without network,
# read-only, with no capabilities, as user 65534, talking UCI (USI for shogi) over stdin and
# stdout.
#
# The image holds no shell and no package manager: only /fairy-stockfish. Fairy-Stockfish is
# GPL-3.0; its source is the tag named here, unchanged.

FROM docker.io/library/alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS build
ARG TARGETARCH
RUN apk add --no-cache build-base curl
WORKDIR /src
RUN curl -fsSLo fsf.tar.gz https://github.com/fairy-stockfish/Fairy-Stockfish/archive/refs/tags/fairy_sf_14_0_1_xq.tar.gz \
 && echo "53914fc89d89afca7cfcfd20660ccdda125f1751f59a68b1f3ed1d4eb6cfe805  fsf.tar.gz" | sha256sum -c - \
 && tar xzf fsf.tar.gz --strip-components=1 && rm fsf.tar.gz
# Any CPU of its kind: SSE4.1 and POPCNT on amd64, plain ARMv8 on arm64.
RUN cd src \
 && arch=$([ "$TARGETARCH" = arm64 ] && echo armv8 || echo x86-64-modern) \
 && make -j"$(nproc)" build ARCH="$arch" COMP=gcc largeboards=yes all=yes EXTRALDFLAGS=-static EXTRACXXFLAGS=-U_FORTIFY_SOURCE \
 && strip stockfish \
 && ./stockfish bench 16 1 6 >/dev/null 2>&1 \
 && mv stockfish fairy-stockfish

FROM scratch
COPY --from=build /src/src/fairy-stockfish /fairy-stockfish
COPY --from=build /src/Copying.txt /COPYING
LABEL org.opencontainers.image.title="bragd-fairy-stockfish" \
      org.opencontainers.image.description="Fairy-Stockfish 14.0.1 with large boards, static, for Bragd's chess Variants, xiangqi, janggi and shogi" \
      org.opencontainers.image.version="14.0.1" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="GPL-3.0-only"
USER 65534:65534
ENTRYPOINT ["/fairy-stockfish"]
