#!/usr/bin/env bash
#MISE description="cargo check xberg-dart + xberg-swift for iOS targets (macOS only)"
#MISE depends=["os:require darwin"]
set -euo pipefail

rustup target add aarch64-apple-ios aarch64-apple-ios-sim
for package in xberg-dart xberg-swift; do
  for target in aarch64-apple-ios aarch64-apple-ios-sim; do
    cargo check --locked -p "$package" --target "$target"
  done
done
