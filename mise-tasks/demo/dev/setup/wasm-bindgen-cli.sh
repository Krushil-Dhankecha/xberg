#!/usr/bin/env bash
#MISE description="Install wasm-bindgen-cli at the version pinned in Cargo.lock"
set -euo pipefail

# shellcheck source=mise-tasks/.lib/cargo-install-pinned.sh
. "$MISE_CONFIG_ROOT/mise-tasks/.lib/cargo-install-pinned.sh"
version="$(grep -A 3 '^name = "wasm-bindgen"$' Cargo.lock | grep '^version' | head -1 | awk '{print $3}' | tr -d '"')"
cargo_install_pinned wasm-bindgen wasm-bindgen-cli "$version"
