# Build on the build host's own architecture and cross-compile to a static musl binary with zig,
# so the arm64 image is not compiled under QEMU.
FROM --platform=$BUILDPLATFORM rust:1-bookworm AS build
ARG TARGETARCH
RUN apt-get update && apt-get install -y --no-install-recommends python3-pip \
    && rm -rf /var/lib/apt/lists/* \
    && pip3 install --no-cache-dir --break-system-packages ziglang \
    && cargo install --locked cargo-zigbuild
WORKDIR /src
# The core embeds repo-root files by relative path (../../../gamedata.json), so keep the repo layout.
COPY gamedata.json stat_map.json stat_names.json names.json location_names.json achievements.json dashboard.html worldgen.js ./
COPY fonts fonts
COPY client client
WORKDIR /src/client
RUN case "$TARGETARCH" in \
      amd64) T=x86_64-unknown-linux-musl ;; \
      arm64) T=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported arch $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && rustup target add "$T" \
    && cargo zigbuild --locked --release -p tracker-server --target "$T" \
    && cp "target/$T/release/tracker-server" /tracker-server

FROM gcr.io/distroless/static-debian12
WORKDIR /app
COPY --from=build /tracker-server /app/tracker-server
EXPOSE 8080
CMD ["/app/tracker-server", "--port", "8080", "--state", "/app/state"]
