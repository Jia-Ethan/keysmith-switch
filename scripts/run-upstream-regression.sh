#!/usr/bin/env bash
# Read-only contract regression against audited Keysmith trees.
# Does not modify those repositories.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
AUDIT="$ROOT/work/source-audit-20260819"

if [[ ! -d "$AUDIT" ]]; then
  echo "audit trees missing: $AUDIT"
  exit 1
fi

run_one() {
  local name="$1"
  local dir="$2"
  echo "== $name =="
  (
    cd "$dir"
    python3 -m py_compile ./*.py
    python3 -m pytest tests -q
  )
}

run_one "claude-keysmith" "$AUDIT/claude-keysmith"
run_one "codex-keysmith" "$AUDIT/codex-keysmith"
run_one "grok-keysmith" "$AUDIT/grok-keysmith"
run_one "zcode-keysmith" "$AUDIT/zcode-keysmith"
