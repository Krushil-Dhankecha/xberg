#!/usr/bin/env bash
#MISE description="Build integration Python sdists and wheels"
set -euo pipefail

# shellcheck source=mise-tasks/integrations/.lib/packages.sh
. "$MISE_CONFIG_ROOT/mise-tasks/integrations/.lib/packages.sh"
# shellcheck disable=SC2154 # py_build_dirs is defined by the sourced packages helper
for package in "${py_build_dirs[@]}"; do
  (
    cd "$package"
    uv build --sdist --wheel
  )
done
