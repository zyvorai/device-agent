# syntax=docker/dockerfile:1.7
FROM rust:1.98-bookworm AS backend
WORKDIR /src
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release --locked || cargo build --release

FROM node:22-bookworm-slim AS dashboard
WORKDIR /src/web/dashboard
COPY web/dashboard/package*.json ./
RUN npm install --no-audit --no-fund
COPY web/dashboard ./
RUN npm run build

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates iproute2 && rm -rf /var/lib/apt/lists/*
COPY --from=backend /src/target/release/zyvor-device-agent /usr/bin/zyvor-device-agent
COPY --from=dashboard /src/web/dashboard/dist /usr/share/zyvor-device-agent/dashboard
COPY config/device-agent.example.toml /etc/zyvor/device-agent.toml
COPY profiles /etc/zyvor/device-agent/profiles
EXPOSE 9188
ENTRYPOINT ["/usr/bin/zyvor-device-agent"]
CMD ["serve"]
