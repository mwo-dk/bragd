# Bragd's ElephantEye image: ElephantEye 3.3 (xqbase, March 2012) built from its source at a
# fixed commit (checked by its digest), linked statically, alone in an otherwise empty image. Built
# for amd64 and arm64 and pushed to ghcr.io/mwo-dk/bragd-elephanteye. Bragd runs it for xiangqi
# when no ElephantEye is installed (crates/bragd-engines), without network, read-only, with no
# capabilities, as user 65534, talking UCCI over stdin and stdout.
#
# It is the only engine Bragd builds that speaks UCCI, so it is what tests the UCCI dialect against
# a real program. The source is xqbase's eleeye repository at the commit named below. Its
# makefile.sh is a g++ line for Windows; this builds the same files. The
# opening book (BOOK.DAT) is left out: ElephantEye plays without it when the file is missing. It
# spins when its input ends without `quit`, so the smoke test sends one.
#
# The image holds no shell and no package manager: only /elephanteye and its licence.
# ElephantEye is LGPL-2.1-or-later; its source is the commit named here, unchanged.

FROM docker.io/library/alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS build
RUN apk add --no-cache build-base curl
WORKDIR /src
RUN curl -fsSLo eleeye.tar.gz https://codeload.github.com/xqbase/eleeye/tar.gz/61c2f8b45c1acf8ec99326746ca0f8ab9bc8305f \
 && echo "a18ec4850242d790b6791215dd2e94486d9a4c8a483738843962faccf83f183a  eleeye.tar.gz" | sha256sum -c - \
 && tar xzf eleeye.tar.gz --strip-components=1 && rm eleeye.tar.gz
# Plain C++ with no CPU-specific code, so the same flags serve amd64 and arm64. Alpine's headers
# check calls at run time, and ElephantEye's input reader copies a line out of its buffer with an
# overlapping memcpy (base/pipe.cpp, GetBuffer), which they stop at once with an illegal
# instruction: _FORTIFY_SOURCE is off for it.
RUN cd eleeye \
 && g++ -DNDEBUG -U_FORTIFY_SOURCE -O2 -static -s -o elephanteye ../base/pipe.cpp ucci.cpp pregen.cpp position.cpp genmoves.cpp hash.cpp book.cpp movesort.cpp preeval.cpp evaluate.cpp search.cpp eleeye.cpp \
 && { { printf 'ucci\nposition startpos\ngo depth 6\n'; sleep 3; echo quit; } | timeout 30 ./elephanteye | grep -q '^bestmove' || exit 1; }

FROM scratch
COPY --from=build /src/eleeye/elephanteye /elephanteye
COPY --from=build /src/LICENSE /COPYING
LABEL org.opencontainers.image.title="bragd-elephanteye" \
      org.opencontainers.image.description="ElephantEye 3.3, static, for Bragd's xiangqi engine when none is installed" \
      org.opencontainers.image.version="3.3" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="LGPL-2.1-or-later"
USER 65534:65534
ENTRYPOINT ["/elephanteye"]
