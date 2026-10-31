# Bragd's Arasan image: Arasan 26.1 built from its released source (checked by its digest),
# with Fathom, its Syzygy prober, at the commit the release names (checked by its digest),
# linked statically, with its evaluation network beside it, alone in an otherwise empty image.
# Built for amd64 and arm64 and pushed to ghcr.io/mwo-dk/bragd-arasan. Bragd runs it for chess
# when no Arasan is installed (crates/bragd-engines), without network, read-only, with no
# capabilities, as user 65534, talking UCI over stdin and stdout.
#
# Arasan loads its network at run time from the folder it runs from, so the network the release
# ships (network/arasanv8-20261005.nnue) sits beside /arasan. Arasan's licence covers its
# network directory too.
#
# The image holds no shell and no package manager: only /arasan, its network and its licence.
# Arasan and Fathom are MIT; the source is the release named here, unchanged.

FROM docker.io/library/alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS build
ARG TARGETARCH
RUN apk add --no-cache build-base curl bc gawk
WORKDIR /src
RUN curl -fsSLo arasan.tar.gz https://github.com/jdart1/arasan-chess/archive/refs/tags/v26.1.tar.gz \
 && echo "7fd33137fc70a5ef27ef9022e941a2139da8a28b2a115657129be4acf9bbd7e1  arasan.tar.gz" | sha256sum -c - \
 && tar xzf arasan.tar.gz --strip-components=1 && rm arasan.tar.gz \
 && curl -fsSLo fathom.tar.gz https://github.com/jdart1/Fathom/archive/c9c6fef0dddc05d2e242c183acf5833149ab676d.tar.gz \
 && echo "ba4553045d796f0521a8554ef21bad44b20fd4a7657eb08ff85d17c9f27ff8ac  fathom.tar.gz" | sha256sum -c - \
 && tar xzf fathom.tar.gz -C src/syzygy --strip-components=1 && rm fathom.tar.gz
# Any CPU of its kind: the makefile's modern build on amd64 (SSE4.1, SSSE3 and POPCNT), its
# NEON build on arm64, where gcc needs -flax-vector-conversions for its NEON code, as clang
# does not. Linked by the default linker: gold, which the makefile asks for, cannot link
# statically against musl.
RUN cd src \
 && type=$([ "$TARGETARCH" = arm64 ] && echo neon || echo modern) \
 && lax=$([ "$TARGETARCH" = arm64 ] && echo -flax-vector-conversions || true) \
 && make -j"$(nproc)" BUILD_TYPE="$type" CC="g++ $lax" LDFLAGS="\$(OPT) -static" \
 && mv ../bin/arasanx-64-"$type" /arasan \
 && strip /arasan \
 && cp ../network/arasanv8-20261005.nnue / \
 && { /arasan bench >bench.txt 2>&1 || { cat bench.txt; exit 1; }; }

FROM scratch
COPY --from=build /arasan /arasanv8-20261005.nnue /
COPY --from=build /src/LICENSE /COPYING
LABEL org.opencontainers.image.title="bragd-arasan" \
      org.opencontainers.image.description="Arasan 26.1 with its network, static, for Bragd's chess engine when none is installed" \
      org.opencontainers.image.version="26.1" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="MIT"
USER 65534:65534
ENTRYPOINT ["/arasan"]
