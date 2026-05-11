#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../.." && pwd)

SOURCE_DIR=${SOURCE_DIR:-$REPO_ROOT/}
TARGET_DIR=${TARGET_DIR:-/mnt/h/matrisaver/}

rsync -a --delete --exclude .venv --exclude __pycache__ --exclude '*.pyc' "$SOURCE_DIR" "$TARGET_DIR"
echo "Synced to $TARGET_DIR"
