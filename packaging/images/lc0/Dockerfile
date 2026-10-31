# Bragd's Leela Chess Zero image: Lc0 0.32.1 built from its released source (checked by its
# digest), with lczero-common at the commit the release names (checked by its digest), for the
# CPU (the Eigen backend: no GPU needed), linked statically, alone in an otherwise empty image.
# Built for amd64 and arm64 and pushed to ghcr.io/mwo-dk/bragd-lc0. Bragd runs it for chess
# when no Lc0 is installed (crates/bragd-engines), without network, read-only, with no
# capabilities, as user 65534, talking UCI over stdin and stdout.
#
# The image holds no network: Lc0 plays only with one, and Bragd downloads it, with the user's
# consent, into its data folder, and mounts it read-only into the container
# (`bragd_engines::network`), setting WeightsFile to it. The build stage proves the binary works
# with Lc0's random backend, which needs no network; Lc0 stops searching when its input ends,
# so the input stays open until it has answered.
#
# Abseil comes from the meson wrap in the release, which checks it by its digest.
#
# The image holds no shell and no package manager: only /lc0 and its licence. Lc0 is
# GPL-3.0-or-later; its source is the release named here, unchanged.

FROM docker.io/library/alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS build
ARG TARGETARCH
RUN apk add --no-cache build-base curl meson python3 eigen-dev linux-headers zlib-dev zlib-static
WORKDIR /src
RUN curl -fsSLo lc0.tar.gz https://github.com/LeelaChessZero/lc0/archive/refs/tags/v0.32.1.tar.gz \
 && echo "78b17e8e8d29da30492c86fdac69c519816f4a9d9c3b787dc41a6f9e8d20fde3  lc0.tar.gz" | sha256sum -c - \
 && tar xzf lc0.tar.gz --strip-components=1 && rm lc0.tar.gz \
 && curl -fsSLo common.tar.gz https://github.com/LeelaChessZero/lczero-common/archive/b326b154221a6eb91977bdccf11d6f89e8547875.tar.gz \
 && echo "d12ec2257c0412a8962c0c439170ce98c6001ab6be6a6444a849d0115bfb330a  common.tar.gz" | sha256sum -c - \
 && tar xzf common.tar.gz -C libs/lczero-common --strip-components=1 && rm common.tar.gz
# Any CPU of its kind: x86-64-v2 (SSE4.2 and POPCNT, no F16C) on amd64, plain ARMv8 on arm64.
# Alpine's fortified headers do not go with link-time optimisation, so they are left out.
# musl gives a new thread 128 KB of stack; the search threads get 8 MB, as glibc gives them.
RUN arch=$([ "$TARGETARCH" = arm64 ] || echo "'-march=x86-64-v2', ") \
 && meson setup build --buildtype=release --prefer-static -Ddefault_library=static \
      -Dcpp_args="[$arch'-U_FORTIFY_SOURCE']" -Dcpp_link_args="['-static', '-Wl,-z,stack-size=8388608']" \
      -Dnative_arch=false -Df16c=false -Dpext=false -Dispc=false -Dopenblas=false \
      -Donnx=false -Dgtest=false -Dplain_cuda=false -Dmetal=disabled \
 && ninja -C build lc0 \
 && strip build/lc0 \
 && touch smoke.txt \
 && { printf 'uci\nsetoption name Backend value random\ngo nodes 100\n'; \
      for i in $(seq 60); do grep -q '^bestmove' smoke.txt && break; sleep 1; done; echo quit; } \
    | build/lc0 >smoke.txt 2>&1 \
 && { grep -q '^bestmove' smoke.txt || { cat smoke.txt; exit 1; }; }

FROM scratch
COPY --from=build /src/build/lc0 /lc0
COPY --from=build /src/COPYING /COPYING
LABEL org.opencontainers.image.title="bragd-lc0" \
      org.opencontainers.image.description="Leela Chess Zero 0.32.1 for the CPU, static, without a network, for Bragd's chess engine when none is installed" \
      org.opencontainers.image.version="0.32.1" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="GPL-3.0-or-later"
USER 65534:65534
ENTRYPOINT ["/lc0"]
