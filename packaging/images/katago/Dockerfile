# Bragd's KataGo image: KataGo 1.18.2 built from its released source (checked by its digest)
# for the CPU (the Eigen backend: no GPU needed), linked statically, with the g170e b15c192
# network (checked by its digest) and a GTP config of its own, alone in an otherwise empty
# image. Built for amd64 and arm64 and pushed to ghcr.io/mwo-dk/bragd-katago. Bragd runs it for
# go when no KataGo is installed (crates/bragd-engines), without network, read-only (its logs go
# to /tmp, which is in memory), with no capabilities, as user 65534, talking GTP over stdin and
# stdout. Its entrypoint starts GTP with that network and config.
#
# The image holds no shell and no package manager. KataGo is MIT; its source is the release
# named here, unchanged. The network is one of KataGo's own released ones.

FROM docker.io/library/alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS build
RUN apk add --no-cache build-base cmake curl eigen-dev zlib-dev zlib-static
WORKDIR /src
RUN curl -fsSLo kg.tar.gz https://github.com/lightvector/KataGo/archive/refs/tags/v1.18.2.tar.gz \
 && echo "fefd554bfaf0daa00d97ac3a767c7aef8d303545b135b0fbf8bd6837772f8bf9  kg.tar.gz" | sha256sum -c - \
 && tar xzf kg.tar.gz --strip-components=1 && rm kg.tar.gz
RUN curl -fsSLo /net.txt.gz https://github.com/lightvector/KataGo/releases/download/v1.3.2/g170e-b15c192-s1672170752-d466197061.txt.gz \
 && echo "09456899f1b9155217f4ca059cf8a68f79b7eba22cf6b7c00ffb7f17ce067967  /net.txt.gz" | sha256sum -c -
# No AVX2, so it runs on any CPU of its kind; no libzip (only training needs it). musl's headers
# that KataGo includes do not pull in <endian.h>, where BYTE_ORDER is (glibc's do).
# zlib is named by its static archive: CMake would pick libz.so, which a static link refuses.
RUN cd cpp \
 && cmake . -DCMAKE_CXX_FLAGS="-include endian.h" -DZLIB_LIBRARY=/usr/lib/libz.a -DUSE_BACKEND=EIGEN -DUSE_AVX2=0 -DNO_GIT_REVISION=1 -DNO_LIBZIP=1 -DCMAKE_BUILD_TYPE=Release -DCMAKE_EXE_LINKER_FLAGS=-static \
 && make -j"$(nproc)" \
 && strip katago \
 && ./katago version
# Logs in memory, one search thread per core up to four.
RUN sed -e 's|^logDir *=.*|logDir = /tmp/katago-logs|' -e 's|^# *numSearchThreads *=.*|numSearchThreads = 4|' cpp/configs/gtp_example.cfg > /gtp.cfg

FROM scratch
COPY --from=build /src/cpp/katago /katago
COPY --from=build /net.txt.gz /net.txt.gz
COPY --from=build /gtp.cfg /gtp.cfg
COPY --from=build /src/LICENSE /LICENSE
LABEL org.opencontainers.image.title="bragd-katago" \
      org.opencontainers.image.description="KataGo 1.18.2 for the CPU, static, with a b15 network, for Bragd's go engine when none is installed" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="MIT"
USER 65534:65534
ENTRYPOINT ["/katago", "gtp", "-model", "/net.txt.gz", "-config", "/gtp.cfg"]
