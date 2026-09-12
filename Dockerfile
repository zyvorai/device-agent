# syntax=docker/dockerfile:1.7
FROM rust:1.98-bookworm AS backend
WORKDIR /src
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release --locked || cargo build --release

FROM node:26-bookworm-slim AS dashboard
WORKDIR /src/web/dashboard
COPY web/dashboard/package*.json ./
RUN npm install --no-audit --no-fund
COPY web/dashboard ./
RUN npm run build

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates iproute2 curl && rm -rf /var/lib/apt/lists/* \
    && groupadd --system zyvor && useradd --system --gid zyvor --no-create-home --shell /usr/sbin/nologin zyvor
COPY --from=backend /src/target/release/zyvor-device-agent /usr/bin/zyvor-device-agent
COPY --from=dashboard /src/web/dashboard/dist /usr/share/zyvor-device-agent/dashboard
COPY config/device-agent.example.toml /etc/zyvor/device-agent.toml
COPY profiles /etc/zyvor/device-agent/profiles
EXPOSE 9188
# Non-root by default: this image has no filesystem state (AppState is entirely
# in-memory) so it only ever needs to read the files above. Real GPIO/I2C/CAN bus
# access on bare metal goes through the systemd deployment (packaging/systemd),
# which intentionally stays root for that reason - see docs/HARDWARE_PERMISSIONS.md.
# Run with --user root (and the relevant --device/--privileged flags) if this
# container itself needs real bus access.
USER zyvor
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -sf http://127.0.0.1:9188/api/v1/ready || exit 1
ENTRYPOINT ["/usr/bin/zyvor-device-agent"]
CMD ["serve"]
