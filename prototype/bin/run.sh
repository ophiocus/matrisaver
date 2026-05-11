#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../.." && pwd)

PYTHON_BIN=${PYTHON_BIN:-python3}
VENV_PATH=${VENV_PATH:-$REPO_ROOT/prototype/.venv}
MATRISAVER_OVERLAY_RECT=${MATRISAVER_OVERLAY_RECT:-0,0,2560,1440}
MATRISAVER_WINDOW_POS=${MATRISAVER_WINDOW_POS:--1080,-489}
MATRISAVER_VIRTUAL_BOUNDS=${MATRISAVER_VIRTUAL_BOUNDS:--1080,-489,5560,1929}

if [ -x "$VENV_PATH/bin/python" ]; then
  PYTHON_BIN="$VENV_PATH/bin/python"
fi

if ! command -v "$PYTHON_BIN" >/dev/null 2>&1; then
  echo "Error: python3 is required. Install it with 'sudo apt install python3'." >&2
  exit 1
fi

if ! "$PYTHON_BIN" -c "import pygame" >/dev/null 2>&1; then
  echo "Error: pygame is not installed." >&2
  echo "Install with: $PYTHON_BIN -m pip install -r $REPO_ROOT/prototype/requirements.txt" >&2
  exit 1
fi

export MATRISAVER_OVERLAY_RECT
export MATRISAVER_WINDOW_POS
export MATRISAVER_VIRTUAL_BOUNDS
exec "$PYTHON_BIN" "$REPO_ROOT/prototype/src/main.py" "$@"
