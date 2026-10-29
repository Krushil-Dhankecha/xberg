#!/usr/bin/env bash
#MISE description="Test one Node integration against a locally built Node binding"
#USAGE flag "--package <package>" env="PACKAGE" required=#true help="Package under integrations/node, e.g. langchain-xberg"
#USAGE flag "--binding-dir <dir>" env="BINDING_DIR" required=#true help="Directory holding the locally built Node binding"
# shellcheck disable=SC2154 # usage_* are set by mise from the #USAGE lines
set -euo pipefail

cd "integrations/node/$usage_package"
npm pkg delete optionalDependencies --prefix "$usage_binding_dir"
npm pkg set "dependencies.@xberg-io/xberg=file:$usage_binding_dir"
npm install --no-audit --no-fund
npm run lint --if-present
npm run build --if-present
npm test --if-present
