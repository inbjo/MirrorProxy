ARG NODE_VERSION=24-bookworm-slim
ARG RUST_VERSION=1.93-bookworm
ARG BASE_IMAGE_REGISTRY=docker.io/library

FROM --platform=$BUILDPLATFORM ${BASE_IMAGE_REGISTRY}/node:${NODE_VERSION} AS web-build
WORKDIR /app/web
COPY web/package.json web/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci
COPY web/ ./
RUN npm run build

FROM ${BASE_IMAGE_REGISTRY}/rust:${RUST_VERSION} AS server-build
WORKDIR /app
ARG GIT_COMMIT=unknown
ARG BUILD_TIME=unknown
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY --from=web-build /app/web/dist web/dist
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    GIT_COMMIT="${GIT_COMMIT}" BUILD_TIME="${BUILD_TIME}" \
    cargo build --locked --release --package mirrorproxy-server --bin mirrorproxy-server && \
    install -D -m 0755 target/release/mirrorproxy-server /out/mirrorproxy-server

FROM ${BASE_IMAGE_REGISTRY}/debian:bookworm-slim AS runtime
ARG UID=10001
ARG GID=10001
RUN apt-get update && \
    apt-get install --yes --no-install-recommends ca-certificates curl tini && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd --gid "${GID}" mirrorproxy && \
    useradd --uid "${UID}" --gid "${GID}" --no-create-home --home-dir /data \
      --shell /usr/sbin/nologin mirrorproxy && \
    install -d -o "${UID}" -g "${GID}" /data /data/cache /etc/mirrorproxy

ARG VERSION=dev
ARG GIT_COMMIT=unknown
ARG BUILD_TIME=unknown
LABEL org.opencontainers.image.title="MirrorProxy" \
      org.opencontainers.image.description="Self-hosted multi-ecosystem mirror proxy" \
      org.opencontainers.image.url="https://github.com/inbjo/MirrorProxy" \
      org.opencontainers.image.source="https://github.com/inbjo/MirrorProxy" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${GIT_COMMIT}" \
      org.opencontainers.image.created="${BUILD_TIME}"

COPY --from=server-build /out/mirrorproxy-server /usr/local/bin/mirrorproxy-server
COPY config.example.toml /etc/mirrorproxy/config.example.toml

ENV MIRRORPROXY_LISTEN_ADDR="0.0.0.0:3000" \
    MIRRORPROXY_DB="/data/mirrorproxy.sqlite3" \
    MIRRORPROXY_CACHE_DIRECTORY="/data/cache" \
    RUST_LOG="mirrorproxy_server=info,tower_http=info"

WORKDIR /data
VOLUME ["/data"]
EXPOSE 3000
USER mirrorproxy
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["curl", "--fail", "--silent", "--show-error", "http://127.0.0.1:3000/healthz"]
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/mirrorproxy-server"]
CMD ["serve"]
