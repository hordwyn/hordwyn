# Multi-stage build. Build context: repository root.
#   docker build -f src/docker/Dockerfile .
FROM rust:1.99.1-slim-trixie AS build
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends \
      curl ca-certificates pkg-config libssl-dev >/dev/null && rm -rf /var/lib/apt/lists/*
WORKDIR /repo
COPY src/sh/ci-tools.sh src/sh/ci-tools.sh
RUN sh src/sh/ci-tools.sh leptos wasm-bindgen tailwind
COPY src/rust src/rust
COPY config config
WORKDIR /repo/src/rust
RUN rustup show active-toolchain || rustup toolchain install
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/repo/src/rust/target \
    if grep -q '\[\[workspace.metadata.leptos\]\]' Cargo.toml; then \
      cargo leptos build --release && mkdir -p /out/site && cp -r target/site/. /out/site/; \
    else \
      cargo build --release -p server && mkdir -p /out/site; \
    fi \
    && cp target/release/server /out/server

FROM debian:trixie-slim AS runtime
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends ca-certificates curl >/dev/null \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /data app && mkdir -p /data && chown app /data
COPY --from=build /out/server /usr/local/bin/server
COPY --from=build /out/site /srv/site
COPY config/brand /etc/app/brand
ENV APP__SERVER__SITE_ROOT=/srv/site \
    APP__BRAND__DIR=/etc/app/brand/default \
    APP__STORAGE__FS_ROOT=/data
USER app
EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/server"]
CMD ["--role=all"]
