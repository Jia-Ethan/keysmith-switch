#!/usr/bin/env bash
# Launch the packaged Preview app with a throwaway HOME. Does not read or write
# the operator's real ~/.claude ~/.codex ~/.grok ~/.zcode-keysmith or
# ~/.keysmith-switch.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="${1:-$ROOT/src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Keysmith Switch.app}"
BIN="$APP/Contents/MacOS/keysmith-switch"
ISO="${KS_ISO:-$(mktemp -d /tmp/ks-iso.XXXX)}"
mkdir -p "$ISO/.claude/keysmith" "$ISO/.keysmith-switch/prompts/claude"
cat > "$ISO/.claude/keysmith/hello.md" <<'MD'
# Native Claude
preview import only
MD
cat > "$ISO/.keysmith-switch/prompts/claude/demo-inactive.md" <<'MD'
---
id: demo-inactive
tool: claude
title: 示例未激活
tags: [demo]
version: 1
deleted: false
---
未激活提示词正文。
MD
echo "ISO=$ISO"
echo "python on PATH?"
command -v python3 || echo "python3 not found"
exec env -i \
  HOME="$ISO" \
  USER="$USER" \
  LOGNAME="$USER" \
  TMPDIR="${TMPDIR:-/tmp}" \
  PATH="/bin:/usr/sbin:/sbin" \
  KEYSMITH_SWITCH_HOME="$ISO/.keysmith-switch" \
  KEYSMITH_SWITCH_TOOL_HOME="$ISO" \
  KEYSMITH_SWITCH_SCAN_HOME="$ISO" \
  "$BIN"
