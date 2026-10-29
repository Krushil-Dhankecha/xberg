#!/usr/bin/env bash
#MISE description="Clean up large build artifacts to reduce cache size (for CI)"
set -euo pipefail

echo "Cleaning up large build artifacts to reduce cache size..."
for extension in rlib so dylib dll; do
  find target -type f -name "*.${extension}" -size +10M -exec rm -f {} \; 2>/dev/null || true
done
rm -rf target/*/incremental 2>/dev/null || true
echo "Cleanup completed successfully"
du -sh target 2>/dev/null || echo "No target directory found"
