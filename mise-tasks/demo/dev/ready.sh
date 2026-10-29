#!/usr/bin/env bash
#MISE description="Print the ready banner"
set -euo pipefail

cat <<'BANNER'

  ┌─────────────────────────────────────────────────────┐
  │  Demo dev environment ready                         │
  │                                                     │
  │  Open → http://localhost:8001/demo-dev.html         │
  │                                                     │
  │  Assets served from http://localhost:9000           │
  │  (same cross-origin setup as the live CDN)          │
  │                                                     │
  │  Ctrl+C to stop both servers                        │
  └─────────────────────────────────────────────────────┘

BANNER
