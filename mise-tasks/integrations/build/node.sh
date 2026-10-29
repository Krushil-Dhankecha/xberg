#!/usr/bin/env bash
#MISE description="Install and build npm integrations against the published binding"
set -euo pipefail

# shellcheck source=mise-tasks/integrations/.lib/packages.sh
. "$MISE_CONFIG_ROOT/mise-tasks/integrations/.lib/packages.sh"
# shellcheck disable=SC2154 # node_dirs is defined by the sourced packages helper
for package in "${node_dirs[@]}"; do
  (
    cd "$package"
    npm install --no-audit --no-fund
    npm run build
  )
done
