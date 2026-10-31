# Bragd's GNU Backgammon image: GNU Backgammon 1.08.003 built from its released source (checked
# by its digest) without its board window, Python or databases, linked statically, alone in an
# otherwise empty image with its neural networks and bear-off databases. Built for amd64 and
# arm64 and pushed to ghcr.io/mwo-dk/bragd-gnubg. Bragd runs it for backgammon when no GNU
# Backgammon is installed (crates/bragd-engines), without network, read-only, with no
# capabilities, as user 65534, talking its command line over stdin and stdout. Its entrypoint
# starts that command line (`-t -q`).
#
# The image holds no shell and no package manager: only /gnubg and its data in
# /usr/share/gnubg. GNU Backgammon is GPL-3.0-or-later; its source is the release named here,
# unchanged.

FROM docker.io/library/alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS build
ARG TARGETARCH
RUN apk add --no-cache build-base curl pkgconf bison flex glib-dev glib-static gettext-static pcre2-static libffi-dev zlib-static
WORKDIR /src
RUN curl -fsSLo gnubg.tar.gz https://ftp.gnu.org/gnu/gnubg/gnubg-release-1.08.003-sources.tar.gz \
 && echo "6f7d969b13cfff786fba90ff8cc5e5d564b97f4f0aa69afe4f3838f18c445979  gnubg.tar.gz" | sha256sum -c - \
 && tar xzf gnubg.tar.gz --strip-components=1 && rm gnubg.tar.gz
# A build that runs on any CPU of its kind: SSE2 on amd64, plain code on arm64. Only the data a
# command-line game needs is kept: the network weights, the bear-off databases and the match
# equity tables; not the board's textures, sounds and fonts.
RUN simd=$([ "$TARGETARCH" = arm64 ] && echo no || echo sse2) \
 && ./configure --prefix=/usr --without-gtk --without-board3d --without-python --without-sqlite --without-libcurl \
      --enable-simd="$simd" PKG_CONFIG="pkg-config --static" \
 && make -j"$(nproc)" LDFLAGS=-all-static \
 && make install-strip DESTDIR=/out \
 && mkdir -p /data \
 && cp -r /out/usr/share/gnubg/gnubg.wd /out/usr/share/gnubg/gnubg_os0.bd /out/usr/share/gnubg/gnubg_ts0.bd /out/usr/share/gnubg/met /data/

FROM scratch
COPY --from=build /out/usr/bin/gnubg /gnubg
COPY --from=build /data /usr/share/gnubg
COPY --from=build /src/COPYING /COPYING
LABEL org.opencontainers.image.title="bragd-gnubg" \
      org.opencontainers.image.description="GNU Backgammon 1.08.003, static, for Bragd's backgammon engine when none is installed" \
      org.opencontainers.image.version="1.08.003" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="GPL-3.0-or-later"
USER 65534:65534
ENTRYPOINT ["/gnubg", "-t", "-q"]
