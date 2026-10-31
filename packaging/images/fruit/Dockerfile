# Bragd's Fruit image: Fruit 2.1 built from its source (checked by its digest), linked
# statically, alone in an otherwise empty image. Built for amd64 and arm64 and pushed to
# ghcr.io/mwo-dk/bragd-fruit. Bragd runs it for chess when no Fruit is installed
# (crates/bragd-engines), without network, read-only, with no capabilities, as user 65534,
# talking UCI over stdin and stdout.
#
# Fruit's own site is gone, so the source is Debian's copy of the 2.1 release, from the
# permanent snapshot archive; its digest is the one Debian's signed fruit_2.1.dfsg-9.dsc names.
# Debian left out only a prebuilt executable (fruit_21_static). The opening book is left out:
# Fruit plays without it when the file is missing.
#
# The image holds no shell and no package manager: only /fruit and its licence. Fruit is
# GPL-2.0-or-later; its source is the release named here, unchanged.

FROM docker.io/library/alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS build
RUN apk add --no-cache build-base curl
WORKDIR /src
RUN curl -fsSLo fruit.tar.gz https://snapshot.debian.org/archive/debian/20210101T000000Z/pool/main/f/fruit/fruit_2.1.dfsg.orig.tar.gz \
 && echo "9bf6364a15f697ed8f79757e9f0f4e149987ba33693e3b79f2a210a26b9edcc3  fruit.tar.gz" | sha256sum -c - \
 && tar xzf fruit.tar.gz --strip-components=1 && rm fruit.tar.gz
# Plain C++ with no CPU-specific code, so the same flags serve amd64 and arm64.
RUN cd src \
 && make -j"$(nproc)" LDFLAGS="-lm -s -static" \
 && { printf 'uci\nposition startpos\ngo depth 6\n' | ./fruit | grep -q '^bestmove' || exit 1; }

FROM scratch
COPY --from=build /src/src/fruit /fruit
COPY --from=build /src/copying.txt /COPYING
LABEL org.opencontainers.image.title="bragd-fruit" \
      org.opencontainers.image.description="Fruit 2.1, static, for Bragd's chess engine when none is installed" \
      org.opencontainers.image.version="2.1" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="GPL-2.0-or-later"
USER 65534:65534
ENTRYPOINT ["/fruit"]
