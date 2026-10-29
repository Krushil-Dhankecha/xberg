#!/usr/bin/env bash
#MISE description="Download OmniDocBench dataset from HuggingFace (~1.3 GB)"
set -euo pipefail

if [ -f tools/benchmark-harness/datasets/omnidocbench/OmniDocBench.json ]; then
  echo "OmniDocBench is already downloaded"
  exit 0
fi
bash tools/benchmark-harness/scripts/download_omnidocbench.sh
