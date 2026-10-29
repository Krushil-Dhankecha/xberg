#!/usr/bin/env bash
#MISE description="Build and run the benchmark harness pipeline benchmark with one candle OCR backend"
#MISE hide=true
#USAGE arg "<feature>" help="benchmark-harness feature that selects the backend"
# shellcheck disable=SC2154 # usage_* are set by mise from the #USAGE lines
set -euo pipefail

cargo build --locked -p benchmark-harness --features "$usage_feature" --release
cargo run --locked -p benchmark-harness --features "$usage_feature" --release -- \
  pipeline-benchmark --fixtures "tools/benchmark-harness/fixtures"
