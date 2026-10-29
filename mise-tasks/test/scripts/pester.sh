#!/usr/bin/env bash
#MISE description="Run the Pester contract tests for scripts/**/*.ps1"
set -euo pipefail

# The same pin ci-scripts.yaml passes to install-pester, so a local run and a CI run exercise the
# same Pester. ~keep
pester_version="6.2.0"
command -v pwsh >/dev/null || {
  echo "pwsh is required for the Pester suites. Install PowerShell 7 or newer." >&2
  exit 1
}
pwsh -NoProfile -Command \
  "exit [int](@(Get-Module -ListAvailable Pester).Version -notcontains [version]\"$pester_version\")" || {
  echo "Pester $pester_version is missing. Install it with:" \
    "Install-Module Pester -RequiredVersion $pester_version -Scope CurrentUser" >&2
  exit 1
}
# Suites are DISCOVERED under scripts/tests/pester, not listed, so a new one is picked up with no
# registration -- the same property `bats --recursive` gives the shell side. Scoped to that
# directory rather than the whole tree so the walk never reaches .venv or node_modules, and found
# with Get-ChildItem rather than `git ls-files` so a suite works before it is staged.
#
# The count is asserted non-empty: Invoke-Pester with no paths exits 0, so a stale scope would
# report a pass having run nothing.
#
# -PassThru with an explicit exit: a plain Invoke-Pester prints its failures and still returns 0,
# which would leave this task green with a red suite. ~keep
# shellcheck disable=SC2016 # the script is PowerShell, expanded by pwsh, not bash
pwsh -NoProfile -Command '
  $suites = @(Get-ChildItem -Path scripts/tests/pester -Recurse -File -Filter "*.Tests.ps1")
  if ($suites.Count -eq 0) { throw "no *.Tests.ps1 suites found under scripts/tests/pester" }
  exit (Invoke-Pester -Path $suites.FullName -Output Detailed -PassThru).FailedCount'
