# The Bragd server's game service (crates/bragd-server), for the Hetzner host beside Supabase
# (docs/adr/0005-bragd-server-on-hosted-supabase.md). Built from the repository's root:
#
#   podman build -f packaging/server/Dockerfile --ignorefile packaging/server/Dockerfile.dockerignore \
#     -t localhost/bragd-server:dev .
#
# Not published: no workflow builds or pushes it. The owner builds and runs it on the host;
# packaging/server/README.md says how. A static binary with the CA certificates, alone in an
# otherwise empty image, run as user 65534.

FROM docker.io/library/rust:1.99-alpine3.24@sha256:7e9e2d068a107a2adafd19930011178d14a78326053cd4c3a9f7ad3fd049557d AS build
RUN apk add --no-cache musl-dev ca-certificates
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p bragd-server \
 && strip target/release/bragd-server

FROM scratch
COPY --from=build /src/target/release/bragd-server /bragd-server
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
LABEL org.opencontainers.image.title="bragd-server" \
      org.opencontainers.image.description="The Bragd server's game service: referees Games between Bragd players" \
      org.opencontainers.image.source="https://github.com/mwo-dk/bragd" \
      org.opencontainers.image.licenses="MIT"
USER 65534:65534
EXPOSE 8080
ENTRYPOINT ["/bragd-server"]
