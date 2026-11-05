#!/bin/sh
# Installs the pinned build tools without Nix (CI, Dockerfile). Versions must match flake.nix.
# usage: ci-tools.sh <tool>...   tools: deny leptos wasm-bindgen tailwind
set -eu
DENY_VERSION=0.20.2
LEPTOS_VERSION=0.3.11
WASM_BINDGEN_VERSION=0.2.130
TAILWIND_VERSION=4.3.3
BIN="${CARGO_HOME:-$HOME/.cargo}/bin"
mkdir -p "$BIN"
fetch() { curl --proto '=https' --tlsv1.2 -fsSL "$1"; }
for tool in "$@"; do
  case "$tool" in
    deny)
      fetch "https://github.com/EmbarkStudios/cargo-deny/releases/download/${DENY_VERSION}/cargo-deny-${DENY_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
        | tar -xz --strip-components=1 -C "$BIN" "cargo-deny-${DENY_VERSION}-x86_64-unknown-linux-musl/cargo-deny" ;;
    leptos)
      fetch "https://github.com/leptos-rs/cargo-leptos/releases/download/v${LEPTOS_VERSION}/cargo-leptos-x86_64-unknown-linux-gnu.tar.gz" \
        | tar -xz --strip-components=1 -C "$BIN" ;;
    wasm-bindgen)
      fetch "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/${WASM_BINDGEN_VERSION}/wasm-bindgen-${WASM_BINDGEN_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
        | tar -xz --strip-components=1 -C "$BIN" ;;
    tailwind)
      fetch "https://github.com/tailwindlabs/tailwindcss/releases/download/v${TAILWIND_VERSION}/tailwindcss-linux-x64" > "$BIN/tailwindcss"
      chmod +x "$BIN/tailwindcss" ;;
    *) echo "unknown tool: $tool" >&2; exit 1 ;;
  esac
done
