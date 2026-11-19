{
  description = "Self-hosted media tracker - development shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        # Toolchain pinned by src/rust/rust-toolchain.toml (single source of truth).
        rust = pkgs.rust-bin.fromRustupToolchainFile ./src/rust/rust-toolchain.toml;
        devPackages = [
          rust
          pkgs.cargo-leptos
          pkgs.cargo-deny
          # licence notices of the image (DEC-855), MIT OR Apache-2.0; version as in ci-tools.sh
          pkgs.cargo-about
          pkgs.sqlx-cli
          # must match the wasm-bindgen crate version pinned in src/rust/Cargo.toml
          pkgs.wasm-bindgen-cli
          pkgs.binaryen
          pkgs.tailwindcss_4
          pkgs.postgresql_18
          pkgs.meilisearch
          # S3-compatible test server (RustFS, Apache-2.0), see DEC-008 and DEC-202
          pkgs.rustfs
          # signed bucket creation for RustFS (curl --aws-sigv4) in `cargo xtask services`
          pkgs.curl
          pkgs.playwright-driver
          # browser + driver for wasm-bindgen-test (same nixpkgs, matching versions)
          pkgs.chromium
          pkgs.chromedriver
          pkgs.nodejs
          pkgs.pkg-config
          pkgs.openssl
          pkgs.jq
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          packages = devPackages;
          PLAYWRIGHT_BROWSERS_PATH = "${pkgs.playwright-driver.browsers}";
          PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
          LEPTOS_TAILWIND_VERSION = pkgs.tailwindcss_4.version;
        };

        checks.devshell = pkgs.runCommand "devshell-check" { nativeBuildInputs = devPackages; } ''
          cargo --version
          cargo-leptos --version
          wasm-bindgen --version
          tailwindcss --help > /dev/null
          touch $out
        '';

        formatter = pkgs.nixpkgs-fmt;
      });
}
