#!/usr/bin/env bash
#MISE description="cargo check xberg-dart + xberg-ffi for Android ABIs (requires ANDROID_NDK_HOME and cargo-ndk)"
set -euo pipefail

command -v cargo-ndk >/dev/null || {
  echo "cargo-ndk not installed. Run: cargo install cargo-ndk --locked" >&2
  exit 1
}
[ -n "${ANDROID_NDK_HOME:-}" ] || {
  echo "ANDROID_NDK_HOME is not set. Install Android NDK and export ANDROID_NDK_HOME=<path-to-ndk>" >&2
  exit 1
}
for package in xberg-dart xberg-ffi; do
  for target in arm64-v8a x86_64; do
    cargo ndk --target "$target" --platform 21 -- check --locked -p "$package"
  done
done
