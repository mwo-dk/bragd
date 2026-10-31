# Bragd's Ethereal image: Ethereal 14.00 built from its released source (checked by its digest)
# with its hand-crafted evaluation, linked statically, alone in an otherwise empty image. Built
# for amd64 and arm64 and pushed to ghcr.io/mwo-dk/bragd-ethereal. Bragd runs it for chess when
# no Ethereal is installed (crates/bragd-engines), without network, read-only, with no
# capabilities, as user 65534, talking UCI over stdin and stdout.
#
# Ethereal's open source plays with its hand-crafted evaluation; its NNUE networks were never
# published, so the build names none (no EVALFILE) and the image holds none.
#
# The image holds no shell and no package manager: only /ethereal and its licence. Ethereal is
# GPL-3.0-or-later; its source is the release named here, unchanged.

FROM docker.io/library/alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS build
ARG TARGETARCH
RUN apk add --no-cache build-base curl
WORKDIR /src
RUN curl -fsSLo ethereal.tar.gz https://github.com/AndyGrant/Ethereal/archive/refs/tags/v14.00.tar.gz \
 && echo "ca2c578810b0dc9d19543806df1a5482d334f53f960a3192042cb8722953a528  ethereal.tar.gz" | sha256sum -c - \
 && tar xzf ethereal.tar.gz --strip-components=1 && rm ethereal.tar.gz
# Any CPU of its kind: the makefile's own SSSE3 and POPCNT flags on amd64 (its ssse3-popcnt
# release), plain ARMv8 on arm64. Its basic target would build for this machine only.
# musl gives a new thread 128 KB of stack; the search threads get 8 MB, as glibc gives them.
RUN cd src \
 && simd=$([ "$TARGETARCH" = arm64 ] || echo '$(POPCNTFLAGS) $(SSSE3FLAGS)') \
 && make basic CC=gcc CFLAGS="\$(RFLAGS) $simd -Wl,-z,stack-size=8388608" \
 && strip Ethereal \
 && { ./Ethereal bench >bench.txt 2>&1 || { cat bench.txt; exit 1; }; } \
 && mv Ethereal ethereal

FROM scratch
COPY --from=build /src/src/ethereal /ethereal
COPY --from=build /src/LICENSE /COPYING
LABEL org.opencontainers.image.title="bragd-ethereal" \
      org.opencontainers.image.description="Ethereal 14.00, static, for Bragd's chess engine when none is installed" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="GPL-3.0-or-later"
USER 65534:65534
ENTRYPOINT ["/ethereal"]
