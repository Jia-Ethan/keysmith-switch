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
  else
    echo "MISSING sidecar $name" >&2
    exit 1
  fi
done

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
