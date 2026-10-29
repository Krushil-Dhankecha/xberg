#!/usr/bin/env bash
#MISE description="Install Rust toolchain and components"
set -euo pipefail

rustup update stable
rustup component add rustfmt clippy
case "$(uname -s)" in
MINGW* | MSYS* | CYGWIN*) rustup component add llvm-tools ;;
esac
cargo install cargo-edit --locked
cargo --version
rustc --version
