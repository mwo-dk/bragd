# Bragd's Pikafish image: Pikafish 2026-09-06 built from its released source (checked by its
# digest), linked statically, alone in an otherwise empty image. Built for amd64 and arm64 and
# pushed to ghcr.io/mwo-dk/bragd-pikafish. Bragd runs it for xiangqi when no Pikafish is
# installed (crates/bragd-engines), without network, read-only, with no capabilities, as user
# 65534, talking UCI over stdin and stdout.
#
# The image holds no evaluation network. Pikafish's network has its own licence (NNUE-License:
# legal use only, no commercial use without permission), so Bragd does not ship it: on first
# use Bragd shows that licence and, once the user agrees, downloads the release archive named
# below, checks it by its digest, takes pikafish.nnue out of it into its data folder, and mounts
# it read-only into the container (`bragd_engines::network`), setting EvalFile to it.
#
# The build stage still takes the network from the release's own archive, checked by its digest,
# only so `pikafish bench` can prove the binary works; it is not copied into the image. The
# archive is the release's, not the moving `master-net` of Pikafish's Networks repository: a
# newer network does not load in an older Pikafish.
#
# The image holds no shell and no package manager: only /pikafish and its licence. Pikafish is
# GPL-3.0; its source is the release named here, unchanged.

FROM docker.io/library/alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS build
ARG TARGETARCH
RUN apk add --no-cache build-base curl 7zip
WORKDIR /src
RUN curl -fsSLo pf.tar.gz https://github.com/official-pikafish/Pikafish/archive/refs/tags/Pikafish-2026-09-06.tar.gz \
 && echo "4d905f5bb9d96632900aa88505d816e2d5997919e1375077a591cfeb0e6c7a10  pf.tar.gz" | sha256sum -c - \
 && tar xzf pf.tar.gz --strip-components=1 && rm pf.tar.gz
# In src/ before the build, so its own scripts/net.sh fetches nothing.
RUN curl -fsSLo pf.7z https://github.com/official-pikafish/Pikafish/releases/download/Pikafish-2026-09-06/Pikafish.2026-09-06.7z \
 && echo "41952bbfe2520faceb5902c69e6ab4845cc999841d2b49a95cc1be7867a25e5b  pf.7z" | sha256sum -c - \
 && 7z e -osrc pf.7z pikafish.nnue NNUE-License.md >/dev/null && rm pf.7z \
 && echo "7d13d73569a9b571ba0eb20cf1596247bc2a42738967e61afef6482b231e900e  src/pikafish.nnue" | sha256sum -c -
# Any CPU of its kind: SSE4.1 and POPCNT on amd64, plain ARMv8 on arm64.
RUN cd src \
 && arch=$([ "$TARGETARCH" = arm64 ] && echo armv8 || echo x86-64-sse41-popcnt) \
 && make -j"$(nproc)" build ARCH="$arch" COMP=gcc EXTRALDFLAGS=-static EXTRACXXFLAGS=-U_FORTIFY_SOURCE \
 && strip pikafish \
 && { ./pikafish bench 16 1 6 >bench.txt 2>&1 || { cat bench.txt; exit 1; }; }

FROM scratch
COPY --from=build /src/src/pikafish /
COPY --from=build /src/Copying.txt /COPYING
LABEL org.opencontainers.image.title="bragd-pikafish" \
      org.opencontainers.image.description="Pikafish 2026-09-06, static, for Bragd's xiangqi engine when none is installed" \
      org.opencontainers.image.version="2026-09-06" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="GPL-3.0-only"
USER 65534:65534
ENTRYPOINT ["/pikafish"]
