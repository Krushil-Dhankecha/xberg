#!/usr/bin/env bash
#MISE description="Run the Bats contract tests for scripts/**/*.sh"
set -euo pipefail

# Suites load the shared helpers with `bats_load_library xberg-bats`, which searches BATS_LIB_PATH
# and nothing else. CI passes the same directory to run-bats via lib-path. ~keep
export BATS_LIB_PATH="$MISE_CONFIG_ROOT/scripts/tests/lib"
command -v bats >/dev/null || {
  echo "bats is required: run \`mise install\`" >&2
  exit 1
}
test -f scripts/tests/lib/xberg-bats/load.bash || {
  echo "shared Bats helpers are missing; run \`mise run test:scripts:lib:sync\`" >&2
  exit 1
}
bats --recursive --print-output-on-failure scripts/tests/bats
