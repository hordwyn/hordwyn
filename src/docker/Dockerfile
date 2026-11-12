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
# The admin CLI (migrate, reindex, key rotation, escape hatches). Same features for the
# `server` library as the build above (bin-features in Cargo.toml), so it is not compiled twice.
RUN cargo build --release -p cli -p server --bin cli \
      --no-default-features --features server/all-connectors \
    && cp target/release/cli /out/cli

FROM debian:trixie-slim AS runtime
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends ca-certificates curl >/dev/null \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /data app && mkdir -p /data && chown app /data
COPY config/certs/ /usr/local/share/ca-certificates/extra/
RUN update-ca-certificates >/dev/null
COPY --from=build /out/server /usr/local/bin/server
COPY --from=build /out/cli /usr/local/bin/cli
# the image build fails here when the CLI is missing or does not start (OPS-1)
RUN /usr/local/bin/cli help >/dev/null
COPY --from=build /out/site /srv/site
COPY config/brand /etc/app/brand
COPY src/rust/crates/ui/icons/vendor /usr/share/doc/app/icons
ENV LEPTOS_SITE_ROOT=/srv/site \
    LEPTOS_OUTPUT_NAME=app \
    LEPTOS_SITE_ADDR=0.0.0.0:3000 \
    LEPTOS_ENV=PROD \
    APP__BRAND__DIR=/etc/app/brand/default \
    APP__STORAGE__FS_ROOT=/data/blobs \
    APP__STORAGE__FS_LEGACY_ROOT=/data \
    APP__SECRETS__KEY_FILE=/data/secrets.key
# One volume, two things in it: the blob store under /data/blobs and the secrets key next to
# it, never inside it (OPS-3). Images before this layout kept the blobs in /data itself;
# FS_LEGACY_ROOT lets the server move `up`, `img`, `http` and `orig` into /data/blobs at
# start (the server only: the storage commands of `cli` refuse to run until it has).
# Relative default paths (an empty APP__SECRETS__KEY_FILE in an env file) resolve to /data too.
WORKDIR /
USER app
EXPOSE 3000
VOLUME /data
ENTRYPOINT ["/usr/local/bin/server"]
CMD ["--role=all"]
