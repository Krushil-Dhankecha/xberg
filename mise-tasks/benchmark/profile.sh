#!/usr/bin/env bash
#MISE description="Run pipeline-benchmark with flamegraph profiling. Builds with --profile profiling so Rust symbols are resolved."
set -euo pipefail

pipeline="${PIPELINE:-baseline}"
doc_filter="${DOC_FILTER:-pdf}"
profile_dir="flamegraphs/$(git rev-parse --short HEAD)"
export RUST_BACKTRACE=short
cargo build --locked --profile profiling -p xberg-cli --features all
cargo build --locked --profile profiling -p benchmark-harness --features profiling
mkdir -p "$profile_dir"
target/profiling/benchmark-harness pipeline-benchmark \
  --fixtures "tools/benchmark-harness/fixtures" \
  --paths "$pipeline" \
  --doc "$doc_filter" \
  --profile-dir "$profile_dir"
echo "Flamegraph SVGs in $profile_dir/. Run: python3 tools/perf/extract_top_symbols.py $profile_dir/$pipeline.svg"
