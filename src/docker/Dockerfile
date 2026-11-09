# Multi-stage build. Build context: repository root.
#   docker build -f src/docker/Dockerfile .
FROM rust:1.99.1-slim-trixie AS build
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends \
      curl ca-certificates pkg-config libssl-dev >/dev/null && rm -rf /var/lib/apt/lists/*
# extra CA certificates (TLS-inspecting proxies), optional: config/certs/*.crt
COPY config/certs/ /usr/local/share/ca-certificates/extra/
RUN update-ca-certificates >/dev/null
WORKDIR /repo
COPY src/sh/ci-tools.sh src/sh/ci-tools.sh
RUN sh src/sh/ci-tools.sh leptos wasm-bindgen tailwind
COPY src/rust src/rust
# Tailwind input and static files referenced by workspace.metadata.leptos
COPY src/web/style src/web/style
COPY src/web/public src/web/public
COPY config config
WORKDIR /repo/src/rust
ENV SQLX_OFFLINE=true
# Memory: one release rustc of the web crate needs 3-4 GB, the wasm build (fat LTO, one codegen
# unit) more. The frontend and the server are built one after the other, with few parallel jobs,
# so the build fits in about 8 GB. Lower it on small hosts: --build-arg CARGO_BUILD_JOBS=1
ARG CARGO_BUILD_JOBS=2
ENV CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}
RUN rustup show active-toolchain || rustup toolchain install
RUN rustup target add wasm32-unknown-unknown \
    && cargo leptos build --release --frontend-only \
    && cargo leptos build --release --server-only \
    && mkdir -p /out/site && cp -r target/site/. /out/site/ \
    && cp target/release/server /out/server

FROM debian:trixie-slim AS runtime
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends ca-certificates curl >/dev/null \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /data app && mkdir -p /data && chown app /data
COPY config/certs/ /usr/local/share/ca-certificates/extra/
RUN update-ca-certificates >/dev/null
COPY --from=build /out/server /usr/local/bin/server
COPY --from=build /out/site /srv/site
COPY config/brand /etc/app/brand
COPY src/rust/crates/ui/icons/vendor /usr/share/doc/app/icons
ENV LEPTOS_SITE_ROOT=/srv/site \
    LEPTOS_OUTPUT_NAME=app \
    LEPTOS_SITE_ADDR=0.0.0.0:3000 \
    LEPTOS_ENV=PROD \
    APP__BRAND__DIR=/etc/app/brand/default \
    APP__STORAGE__FS_ROOT=/data \
    APP__SECRETS__KEY_FILE=/data/secrets.key
USER app
EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/server"]
CMD ["--role=all"]
