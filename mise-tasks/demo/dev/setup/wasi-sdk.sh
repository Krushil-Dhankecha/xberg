#!/usr/bin/env bash
#MISE description="Install WASI SDK 25 to ~/wasi-sdk-25 (skips if already present)"
set -euo pipefail

version=25
sdk_dir="${WASI_SDK_PATH:-$HOME/wasi-sdk-$version}"
if [ -d "$sdk_dir/share/wasi-sysroot" ]; then
  echo "WASI SDK $version already installed at $sdk_dir"
  exit 0
fi
os="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"
case "$os-$arch" in
linux-x86_64) package="wasi-sdk-$version.0-x86_64-linux.tar.gz" ;;
linux-aarch64) package="wasi-sdk-$version.0-arm64-linux.tar.gz" ;;
darwin-x86_64) package="wasi-sdk-$version.0-x86_64-macos.tar.gz" ;;
darwin-arm64) package="wasi-sdk-$version.0-arm64-macos.tar.gz" ;;
*)
  echo "Unsupported platform: $os-$arch"
  echo "Download manually: https://github.com/WebAssembly/wasi-sdk/releases/tag/wasi-sdk-$version"
  exit 1
  ;;
esac
base_url="https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-$version"
archive="$(mktemp /tmp/wasi-sdk-XXXXXX.tar.gz)"
trap 'rm -f "$archive" "$archive.sha256"' EXIT
echo "Downloading WASI SDK $version ..."
curl -fL "$base_url/$package" -o "$archive"
curl -fL "$base_url/$package.sha256" -o "$archive.sha256"
echo "Verifying SHA256 ..."
expected_hash="$(awk '{print $1}' "$archive.sha256")"
if command -v sha256sum >/dev/null 2>&1; then
  echo "$expected_hash  $archive" | sha256sum -c -
elif command -v shasum >/dev/null 2>&1; then
  echo "$expected_hash  $archive" | shasum -a 256 -c -
else
  echo "Warning: no SHA256 tool found, skipping checksum verification"
fi
echo "Extracting to $sdk_dir ..."
mkdir -p "$sdk_dir"
tar -xzf "$archive" -C "$sdk_dir" --strip-components=1
echo ""
echo "WASI SDK $version installed at $sdk_dir"
echo "Add to your shell profile:"
echo "  export WASI_SDK_PATH=$sdk_dir"
