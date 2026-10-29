#!/usr/bin/env bash
#MISE description="Run an opt-in Xberg baseline/layout warm in-process diagnostic"
set -euo pipefail

pipeline="${PIPELINE:-layout}"
case "$pipeline" in
baseline | layout) ;;
*)
  echo "PIPELINE must be baseline or layout" >&2
  exit 1
  ;;
esac
output_format="${OUTPUT_FORMAT:-markdown}"
export FRAMEWORK="xberg-${output_format}-${pipeline}-steady-state"
export MODE=single-file
export OUTPUT_FORMAT="$output_format"
export MEASURE_QUALITY="${MEASURE_QUALITY:-true}"
exec mise run benchmark:run
