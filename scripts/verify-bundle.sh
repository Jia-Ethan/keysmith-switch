#!/usr/bin/env bash
set -euo pipefail
RESIGN=false
SIGNING_MODE="adhoc"
if [[ "${1:-}" == "--resign-adhoc" ]]; then
  RESIGN=true
  shift
fi
if [[ "${1:-}" == "--require-developer-id" ]]; then
  SIGNING_MODE="developer-id"
  shift
fi
APP="${1:-}"
if [[ -z "$APP" ]]; then
  echo "usage: $0 [--resign-adhoc] path/to/Keysmith Switch.app" >&2
  exit 1
fi
if [[ ! -d "$APP" ]]; then
  echo "app bundle does not exist: $APP" >&2
  exit 1
fi
MACOS="$APP/Contents/MacOS"
PLIST="$APP/Contents/Info.plist"
echo "app: $APP"

assert_arm64_executable() {
  local binary="$1"
  local label="$2"
  if [[ ! -x "$binary" ]]; then
    echo "missing executable $label: $binary" >&2
    exit 1
  fi
  local description architectures
  description="$(file "$binary")"
  architectures="$(lipo -archs "$binary")"
  if [[ "$description" != *"Mach-O"* || "$architectures" != "arm64" ]]; then
    echo "$label is not a thin arm64 Mach-O: $description (architectures: $architectures)" >&2
    exit 1
  fi
  echo "$label ok: $architectures"
}

assert_sidecar_runs() {
  local binary="$1"
  local label="$2"
  local output
  if ! output="$("$binary" --version 2>&1)"; then
    echo "$label runtime smoke failed: $output" >&2
    exit 1
  fi
  if [[ -z "$output" ]]; then
    echo "$label returned an empty version response" >&2
    exit 1
  fi
  echo "$label runtime ok: ${output%%$'\n'*}"
}

assert_sidecar_previews() {
  local smoke_home prompt
  smoke_home="$(mktemp -d "${TMPDIR:-/tmp}/keysmith-bundle-smoke.XXXXXX")"
  prompt="$smoke_home/prompt.md"
  printf '# Bundle smoke\nPreview only.\n' > "$prompt"
  mkdir -p "$smoke_home/.codex"
  : > "$smoke_home/.codex/config.toml"

  HOME="$smoke_home" "$MACOS/keysmith-claude" install --scope user \
    --file "$prompt" --name bundle-smoke --json >/dev/null
  HOME="$smoke_home" "$MACOS/keysmith-codex" --file "$prompt" \
    --name bundle-smoke --dry-run --lang en --codex-dir "$smoke_home/.codex" >/dev/null
  HOME="$smoke_home" "$MACOS/keysmith-grok" --json --file "$prompt" \
    --name bundle-smoke --dry-run --grok-dir "$smoke_home/.grok" >/dev/null
  HOME="$smoke_home" "$MACOS/keysmith-zcode" doctor \
    --managed-dir "$smoke_home/.zcode-keysmith" >/dev/null

  if [[ -e "$smoke_home/.claude" || -e "$smoke_home/.grok" || -e "$smoke_home/.zcode-keysmith" ]]; then
    echo "sidecar preview smoke unexpectedly wrote managed configuration" >&2
    rm -rf "$smoke_home"
    exit 1
  fi
  rm -rf "$smoke_home"
  echo "sidecar preview smoke passed"
}

if [[ ! -f "$PLIST" ]]; then
  echo "missing Info.plist: $PLIST" >&2
  exit 1
fi
identifier="$(plutil -extract CFBundleIdentifier raw -o - "$PLIST")"
if [[ "$identifier" != "com.jia-ethan.keysmith-switch" ]]; then
  echo "unexpected bundle identifier: $identifier" >&2
  exit 1
fi

assert_arm64_executable "$MACOS/keysmith-switch" "main executable"
for name in keysmith-claude keysmith-codex keysmith-grok keysmith-zcode; do
  if [[ -x "$MACOS/$name" ]]; then
    assert_arm64_executable "$MACOS/$name" "sidecar $name"
    assert_sidecar_runs "$MACOS/$name" "sidecar $name"
  else
    echo "MISSING sidecar $name" >&2
    exit 1
  fi
done
assert_sidecar_previews

if [[ "$RESIGN" == true ]]; then
  echo "re-signing app with a local ad-hoc identity"
  codesign --force --deep --sign - "$APP"
fi

codesign --verify --deep --strict --verbose=2 "$APP"
signature="$(codesign --display --verbose=4 "$APP" 2>&1)"
if [[ "$SIGNING_MODE" == "developer-id" ]]; then
  if ! grep -q '^Authority=Developer ID Application:' <<<"$signature"; then
    echo "bundle is not signed with Developer ID Application" >&2
    exit 1
  fi
  if grep -q '^TeamIdentifier=not set$' <<<"$signature"; then
    echo "Developer ID bundle has no TeamIdentifier" >&2
    exit 1
  fi
  spctl --assess --type execute --verbose=2 "$APP"
else
  if ! grep -q '^Signature=adhoc$' <<<"$signature"; then
    echo "bundle is not ad-hoc signed" >&2
    exit 1
  fi
  if ! grep -q '^TeamIdentifier=not set$' <<<"$signature"; then
    echo "bundle unexpectedly has a signing team" >&2
    exit 1
  fi
  if grep -q '^Authority=' <<<"$signature"; then
    echo "bundle unexpectedly contains a production signing authority" >&2
    exit 1
  fi
fi

echo "bundle verification passed"
