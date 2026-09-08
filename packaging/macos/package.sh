#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DIST_DIR="$PROJECT_ROOT/dist"
CHECK_ONLY=false

fail() {
    printf 'macOS packaging error: %s\n' "$1" >&2
    exit 1
}

need_command() {
    command -v "$1" >/dev/null 2>&1 \
        || fail "missing command '$1'; install it manually, then retry"
}

if [[ $# -lt 1 || $# -gt 2 ]]; then
    fail "usage: package.sh {arm64|x86_64} [--check]"
fi
ARCH="$1"
if [[ $# -eq 2 ]]; then
    [[ "$2" == "--check" ]] || fail "usage: package.sh {arm64|x86_64} [--check]"
    CHECK_ONLY=true
fi

case "$ARCH" in
    arm64)
        TARGET="aarch64-apple-darwin"
        FILE_ARCH="arm64"
        ;;
    x86_64)
        TARGET="x86_64-apple-darwin"
        FILE_ARCH="x86_64"
        ;;
    *) fail "usage: package.sh {arm64|x86_64} [--check]" ;;
esac

[[ "$(uname -s)" == "Darwin" ]] \
    || fail "macOS packages must be built on macOS"

for command in cargo rustup python3 xcode-select sips codesign hdiutil plutil file ditto; do
    need_command "$command"
done
xcode-select -p >/dev/null 2>&1 \
    || fail "Xcode command line tools are not configured"
rustup target list --installed | grep -Fxq "$TARGET" \
    || fail "missing Rust target; run: rustup target add $TARGET"

MACOS_BUILD_NUMBER="${MACOS_BUILD_NUMBER:-1}"
MACOS_MIN_VERSION="${MACOS_MIN_VERSION:-11.0}"
[[ "$MACOS_BUILD_NUMBER" =~ ^[0-9]+$ ]] \
    || fail "MACOS_BUILD_NUMBER must contain digits only"
[[ "$MACOS_MIN_VERSION" =~ ^[0-9]+\.[0-9]+([.][0-9]+)?$ ]] \
    || fail "MACOS_MIN_VERSION must look like 11.0 or 11.0.1"

SIGNING_ENABLED=false
NOTARIZATION_ENABLED=false
if [[ -n "${MACOS_SIGN_IDENTITY:-}" ]]; then
    SIGNING_ENABLED=true
fi
if [[ -n "${MACOS_NOTARY_PROFILE:-}" ]]; then
    $SIGNING_ENABLED \
        || fail "MACOS_NOTARY_PROFILE requires MACOS_SIGN_IDENTITY"
    need_command xcrun
    NOTARIZATION_ENABLED=true
fi

METADATA="$(cd "$PROJECT_ROOT" && cargo metadata --no-deps --format-version 1)"
VERSION="$(python3 -c '
import json
import sys
packages = json.load(sys.stdin)["packages"]
matches = [package for package in packages if package["name"] == "legend_shot"]
if len(matches) != 1:
    raise SystemExit("cargo metadata did not return exactly one legend_shot package")
print(matches[0]["version"])
' <<<"$METADATA")"

PREFERRED_ICON="$PROJECT_ROOT/packaging/assets/legend-shot-1024.png"
FALLBACK_ICON="$PROJECT_ROOT/src/icon/tray-focus-128.png"
if [[ -f "$PREFERRED_ICON" ]]; then
    ICON_SOURCE="$PREFERRED_ICON"
    ICON_STATE="release icon"
else
    ICON_SOURCE="$FALLBACK_ICON"
    ICON_STATE="128x128 fallback icon"
fi
[[ -f "$ICON_SOURCE" ]] || fail "application icon is missing"

if $CHECK_ONLY; then
    printf 'macOS packaging preflight passed.\n'
    printf 'Version: %s\n' "$VERSION"
    printf 'Architecture: %s\n' "$ARCH"
    printf 'Target: %s\n' "$TARGET"
    printf 'Minimum macOS: %s\n' "$MACOS_MIN_VERSION"
    printf 'Icon: %s\n' "$ICON_STATE"
    $SIGNING_ENABLED && printf 'Signing: enabled\n' \
        || printf 'Signing: disabled (ad-hoc test package)\n'
    $NOTARIZATION_ENABLED && printf 'Notarization: enabled\n' \
        || printf 'Notarization: disabled\n'
    exit 0
fi

MACOSX_DEPLOYMENT_TARGET="$MACOS_MIN_VERSION" \
    cargo build \
        --manifest-path "$PROJECT_ROOT/Cargo.toml" \
        --release \
        --locked \
        --target "$TARGET"

BINARY="$PROJECT_ROOT/target/$TARGET/release/legend_shot"
[[ -s "$BINARY" ]] || fail "release binary is missing or empty: $BINARY"
file "$BINARY" | grep -Fq "$FILE_ARCH" \
    || fail "release binary does not contain the requested $FILE_ARCH architecture"

STAGE_DIR="$DIST_DIR/packaging-macos-$ARCH"
APP_BUNDLE="$STAGE_DIR/Legend Shot.app"
APP_CONTENTS="$APP_BUNDLE/Contents"
APP_BINARY="$APP_CONTENTS/MacOS/legend_shot"
ICONSET="$STAGE_DIR/AppIcon.iconset"
DMG_ROOT="$STAGE_DIR/dmg-root"
DMG_OUTPUT="$DIST_DIR/LegendShot-${VERSION}-macos-${ARCH}.dmg"

mkdir -p "$DIST_DIR"
rm -rf "$STAGE_DIR"
mkdir -p "$APP_CONTENTS/MacOS" "$APP_CONTENTS/Resources" "$ICONSET"

if [[ "$ICON_SOURCE" == "$FALLBACK_ICON" ]]; then
    printf 'Warning: using the 128x128 tray icon; replace it before public release.\n' >&2
fi
sips -z 16 16 "$ICON_SOURCE" --out "$ICONSET/icon_16x16.png" >/dev/null
sips -z 32 32 "$ICON_SOURCE" --out "$ICONSET/icon_16x16@2x.png" >/dev/null
sips -z 32 32 "$ICON_SOURCE" --out "$ICONSET/icon_32x32.png" >/dev/null
sips -z 64 64 "$ICON_SOURCE" --out "$ICONSET/icon_32x32@2x.png" >/dev/null
sips -z 128 128 "$ICON_SOURCE" --out "$ICONSET/icon_128x128.png" >/dev/null
sips -z 256 256 "$ICON_SOURCE" --out "$ICONSET/icon_128x128@2x.png" >/dev/null
sips -z 256 256 "$ICON_SOURCE" --out "$ICONSET/icon_256x256.png" >/dev/null
sips -z 512 512 "$ICON_SOURCE" --out "$ICONSET/icon_256x256@2x.png" >/dev/null
sips -z 512 512 "$ICON_SOURCE" --out "$ICONSET/icon_512x512.png" >/dev/null
sips -z 1024 1024 "$ICON_SOURCE" --out "$ICONSET/icon_512x512@2x.png" >/dev/null
python3 "$SCRIPT_DIR/make_icns.py" \
    "$ICONSET" \
    "$APP_CONTENTS/Resources/AppIcon.icns"

ditto "$BINARY" "$APP_BINARY"
chmod 755 "$APP_BINARY"
python3 - \
    "$SCRIPT_DIR/Info.plist.in" \
    "$APP_CONTENTS/Info.plist" \
    "$VERSION" \
    "$MACOS_BUILD_NUMBER" \
    "$MACOS_MIN_VERSION" <<'PY'
from pathlib import Path
import sys

template_path, output_path, version, build_number, minimum_version = sys.argv[1:]
content = Path(template_path).read_text(encoding="utf-8")
content = content.replace("@VERSION@", version)
content = content.replace("@BUILD_NUMBER@", build_number)
content = content.replace("@MIN_VERSION@", minimum_version)
if "@" in content:
    raise SystemExit("unresolved placeholder in Info.plist")
Path(output_path).write_text(content, encoding="utf-8")
PY
plutil -lint "$APP_CONTENTS/Info.plist"

if $SIGNING_ENABLED; then
    codesign \
        --force \
        --options runtime \
        --timestamp \
        --sign "$MACOS_SIGN_IDENTITY" \
        "$APP_BUNDLE"
else
    codesign --force --deep --sign - "$APP_BUNDLE"
fi
codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE"

mkdir -p "$DMG_ROOT"
ditto "$APP_BUNDLE" "$DMG_ROOT/Legend Shot.app"
ln -s /Applications "$DMG_ROOT/Applications"
rm -f "$DMG_OUTPUT"
hdiutil create \
    -volname "Legend Shot" \
    -srcfolder "$DMG_ROOT" \
    -ov \
    -format UDZO \
    "$DMG_OUTPUT"
hdiutil verify "$DMG_OUTPUT"

if $SIGNING_ENABLED; then
    codesign \
        --force \
        --timestamp \
        --sign "$MACOS_SIGN_IDENTITY" \
        "$DMG_OUTPUT"
    codesign --verify --verbose=2 "$DMG_OUTPUT"
fi

if $NOTARIZATION_ENABLED; then
    xcrun notarytool submit "$DMG_OUTPUT" \
        --keychain-profile "$MACOS_NOTARY_PROFILE" \
        --wait
    xcrun stapler staple "$DMG_OUTPUT"
    xcrun stapler validate "$DMG_OUTPUT"
fi

[[ -s "$DMG_OUTPUT" ]] || fail "DMG was not created"

printf 'macOS packaging completed for Legend Shot %s.\n' "$VERSION"
printf 'Architecture: %s\n' "$ARCH"
$SIGNING_ENABLED && printf 'Signing: Developer ID\n' \
    || printf 'Signing: ad-hoc test package\n'
$NOTARIZATION_ENABLED && printf 'Notarization: complete\n' \
    || printf 'Notarization: not requested\n'
printf 'DMG: %s\n' "$DMG_OUTPUT"
