#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="${1:-}"
if [[ -z "$APP" ]]; then
  echo "usage: $0 path/to/Keysmith Switch.app" >&2
  exit 1
fi
MACOS="$APP/Contents/MacOS"
echo "app: $APP"
ls -la "$MACOS"
for name in keysmith-claude keysmith-codex keysmith-grok keysmith-zcode; do
  if [[ -x "$MACOS/$name" ]]; then
    echo "sidecar ok $name"
    file "$MACOS/$name"
  else
    echo "MISSING sidecar $name" >&2
    exit 1
  fi
done
echo "bundle verification passed"
