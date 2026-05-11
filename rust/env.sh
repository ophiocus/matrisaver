#!/usr/bin/env bash
set -euo pipefail

# Ensure rustup-managed tools are available in both interactive and non-interactive shells.
if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
fi

case ":${PATH}:" in
  *":$HOME/.cargo/bin:"*) ;;
  *) export PATH="$HOME/.cargo/bin:$PATH" ;;
esac
