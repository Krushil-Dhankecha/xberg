#!/usr/bin/env bash
#MISE description="Test one Python integration against a locally built xberg wheel"
#USAGE flag "--package <package>" env="PACKAGE" required=#true help="Integration under integrations/, e.g. python/crewai"
#USAGE flag "--wheel-dir <dir>" env="WHEEL_DIR" required=#true help="Directory holding the locally built xberg wheel"
# shellcheck disable=SC2154 # usage_* are set by mise from the #USAGE lines
set -euo pipefail

# shellcheck source=mise-tasks/integrations/.lib/packages.sh
. "$MISE_CONFIG_ROOT/mise-tasks/integrations/.lib/packages.sh"
test_python_integration "$usage_package" "$usage_wheel_dir"
