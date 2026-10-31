# Bragd's GNU Go image: GNU Go 3.8 built from its released source (checked by its digest),
# without readline or colour, linked statically, alone in an otherwise empty image. Built for
# amd64 and arm64 and pushed to ghcr.io/mwo-dk/bragd-gnugo. Bragd runs it for go when no GNU Go
# is installed (crates/bragd-engines), without network, read-only, with no capabilities, as user
# 65534, talking GTP over stdin and stdout. Its entrypoint starts GTP.
#
# GNU Go 3.8 predates today's compilers: it needs common symbols and the old C rules for
# implicit declarations, as the distributions build it.
#
# The image holds no shell and no package manager: only /gnugo. GNU Go is GPL-3.0-or-later;
# its source is the release named here, unchanged.

FROM docker.io/library/alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS build
RUN apk add --no-cache build-base curl
WORKDIR /src
RUN curl -fsSLo gnugo.tar.gz https://ftp.gnu.org/gnu/gnugo/gnugo-3.8.tar.gz \
 && echo "da68d7a65f44dcf6ce6e4e630b6f6dd9897249d34425920bfdd4e07ff1866a72  gnugo.tar.gz" | sha256sum -c - \
 && tar xzf gnugo.tar.gz --strip-components=1 && rm gnugo.tar.gz
RUN ./configure --without-readline --disable-color \
      CFLAGS="-O2 -fcommon -std=gnu89 -Wno-error=implicit-function-declaration -Wno-error=implicit-int -Wno-error=incompatible-pointer-types" \
      LDFLAGS=-static \
 && make -j"$(nproc)" \
 && strip interface/gnugo \
 && echo "genmove b" | ./interface/gnugo --mode gtp --level 1 | grep -q '^='

FROM scratch
COPY --from=build /src/interface/gnugo /gnugo
COPY --from=build /src/COPYING /COPYING
LABEL org.opencontainers.image.title="bragd-gnugo" \
      org.opencontainers.image.description="GNU Go 3.8, static, for Bragd's go engine when none is installed" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="GPL-3.0-or-later"
USER 65534:65534
ENTRYPOINT ["/gnugo", "--mode", "gtp"]
