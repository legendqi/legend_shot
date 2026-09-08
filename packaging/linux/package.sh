#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DIST_DIR="$PROJECT_ROOT/dist"
STAGE_DIR="$DIST_DIR/packaging-linux-x86_64"
TARGET="x86_64-unknown-linux-gnu"
CHECK_ONLY=false
PACKAGING_TOOL_CACHE_DIR="${PACKAGING_TOOL_CACHE_DIR:-$PROJECT_ROOT/.local/packaging-tools}"
LINUXDEPLOY_URL="${LINUXDEPLOY_URL:-https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage}"
LINUXDEPLOY_PLUGIN_GTK_URL="${LINUXDEPLOY_PLUGIN_GTK_URL:-https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh}"

if [[ $# -gt 1 ]]; then
    printf 'Usage: %s [--check]\n' "$0" >&2
    exit 1
fi
case "${1:-}" in
    "") ;;
    --check) CHECK_ONLY=true ;;
    *)
        printf 'Usage: %s [--check]\n' "$0" >&2
        exit 1
        ;;
esac

fail() {
    printf 'Linux packaging error: %s\n' "$1" >&2
    exit 1
}

need_command() {
    command -v "$1" >/dev/null 2>&1 \
        || fail "missing command '$1'; install it manually, then retry"
}

libxdo_development_files_exist() {
    pkg-config --exists xdo && return 0

    local include_path
    local library_dir
    for include_path in /usr/include/xdo.h /usr/local/include/xdo.h; do
        [[ -f "$include_path" ]] || continue
        for library_dir in /usr/lib/*-linux-gnu /usr/lib64 /usr/lib /usr/local/lib; do
            [[ -e "$library_dir/libxdo.so" ]] && return 0
        done
    done
    return 1
}

download_packaging_tool() {
    local url="$1"
    local destination="$2"
    local temporary
    mkdir -p "$PACKAGING_TOOL_CACHE_DIR"
    temporary="$(mktemp "$PACKAGING_TOOL_CACHE_DIR/.download.XXXXXX")"
    printf 'Downloading %s\n' "$url" >&2
    if command -v curl >/dev/null 2>&1; then
        if ! curl --location --fail --silent --show-error --output "$temporary" "$url"; then
            rm -f "$temporary"
            fail "failed to download packaging tool: $url"
        fi
    elif command -v wget >/dev/null 2>&1; then
        if ! wget --quiet --output-document="$temporary" "$url"; then
            rm -f "$temporary"
            fail "failed to download packaging tool: $url"
        fi
    else
        rm -f "$temporary"
        fail "curl or wget is required to download Linux packaging tools"
    fi
    [[ -s "$temporary" ]] || {
        rm -f "$temporary"
        fail "downloaded packaging tool is empty: $url"
    }
    chmod 755 "$temporary"
    mv "$temporary" "$destination"
}

resolve_linuxdeploy() {
    local cached="$PACKAGING_TOOL_CACHE_DIR/linuxdeploy-x86_64.AppImage"
    if [[ -n "${LINUXDEPLOY:-}" ]]; then
        printf '%s\n' "$LINUXDEPLOY"
        return
    fi
    if command -v linuxdeploy >/dev/null 2>&1; then
        command -v linuxdeploy
        return
    fi
    if command -v linuxdeploy-x86_64.AppImage >/dev/null 2>&1; then
        command -v linuxdeploy-x86_64.AppImage
        return
    fi
    if [[ ! -x "$cached" ]]; then
        download_packaging_tool "$LINUXDEPLOY_URL" "$cached"
    fi
    printf '%s\n' "$cached"
}

resolve_gtk_plugin() {
    local linuxdeploy_dir="$1"
    local candidate
    local cached="$PACKAGING_TOOL_CACHE_DIR/linuxdeploy-plugin-gtk.sh"
    if [[ -n "${LINUXDEPLOY_PLUGIN_GTK:-}" ]]; then
        printf '%s\n' "$LINUXDEPLOY_PLUGIN_GTK"
        return
    fi
    for candidate in \
        "$(command -v linuxdeploy-plugin-gtk.sh 2>/dev/null || true)" \
        "$(command -v linuxdeploy-plugin-gtk 2>/dev/null || true)" \
        "$linuxdeploy_dir/linuxdeploy-plugin-gtk.sh" \
        "$linuxdeploy_dir/linuxdeploy-plugin-gtk"; do
        if [[ -n "$candidate" && -f "$candidate" ]]; then
            printf '%s\n' "$candidate"
            return
        fi
    done
    if [[ ! -x "$cached" ]]; then
        download_packaging_tool "$LINUXDEPLOY_PLUGIN_GTK_URL" "$cached"
    fi
    printf '%s\n' "$cached"
}

[[ "$(uname -s)" == "Linux" ]] \
    || fail "Linux packages must be built on Linux"
[[ "$(uname -m)" == "x86_64" ]] \
    || fail "this script currently packages x86_64 only"

for command in cargo rustup python3 pkg-config dpkg-deb desktop-file-validate xclip realpath; do
    need_command "$command"
done

rustup target list --installed | grep -Fxq "$TARGET" \
    || fail "missing Rust target; run: rustup target add $TARGET"
pkg-config --exists gtk+-3.0 \
    || fail "GTK3 development files are missing"
libxdo_development_files_exist \
    || fail "libxdo development files are missing"
if ! pkg-config --exists ayatana-appindicator3-0.1 \
    && ! pkg-config --exists appindicator3-0.1; then
    fail "Ayatana AppIndicator or AppIndicator development files are missing"
fi

LINUXDEPLOY_RESOLVED="$(resolve_linuxdeploy)" || exit $?
LINUXDEPLOY_BIN="$(realpath "$LINUXDEPLOY_RESOLVED")"
[[ -f "$LINUXDEPLOY_BIN" && -x "$LINUXDEPLOY_BIN" ]] \
    || fail "linuxdeploy is not executable: $LINUXDEPLOY_BIN"
GTK_PLUGIN_RESOLVED="$(resolve_gtk_plugin "$(dirname "$LINUXDEPLOY_BIN")")" || exit $?
GTK_PLUGIN="$(realpath "$GTK_PLUGIN_RESOLVED")"
[[ -f "$GTK_PLUGIN" && -x "$GTK_PLUGIN" ]] \
    || fail "linuxdeploy GTK plugin is not executable: $GTK_PLUGIN"

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

if $CHECK_ONLY; then
    printf 'Linux packaging preflight passed.\n'
    printf 'Version: %s\n' "$VERSION"
    printf 'Target: %s\n' "$TARGET"
    printf 'linuxdeploy: %s\n' "$LINUXDEPLOY_BIN"
    printf 'GTK plugin: %s\n' "$GTK_PLUGIN"
    exit 0
fi

(cd "$PROJECT_ROOT" && cargo build --release --locked --target "$TARGET")

BINARY="$PROJECT_ROOT/target/$TARGET/release/legend_shot"
[[ -s "$BINARY" ]] || fail "release binary is missing or empty: $BINARY"

mkdir -p "$DIST_DIR"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"

PREFERRED_ICON="$PROJECT_ROOT/packaging/assets/legend-shot-1024.png"
FALLBACK_ICON="$PROJECT_ROOT/src/icon/tray-focus-128.png"
if [[ -f "$PREFERRED_ICON" ]]; then
    ICON_SOURCE="$PREFERRED_ICON"
else
    printf 'Warning: using the 128x128 tray icon; replace it before public release.\n' >&2
    ICON_SOURCE="$FALLBACK_ICON"
fi
[[ -f "$ICON_SOURCE" ]] || fail "application icon is missing"
PACKAGE_ICON="$STAGE_DIR/legend-shot.png"
install -m 644 "$ICON_SOURCE" "$PACKAGE_ICON"

DEB_ROOT="$STAGE_DIR/deb-root"
mkdir -p \
    "$DEB_ROOT/DEBIAN" \
    "$DEB_ROOT/usr/bin" \
    "$DEB_ROOT/usr/share/applications" \
    "$DEB_ROOT/usr/share/icons/hicolor/128x128/apps"
install -m 755 "$BINARY" "$DEB_ROOT/usr/bin/legend_shot"
install -m 644 "$SCRIPT_DIR/legend-shot.desktop" \
    "$DEB_ROOT/usr/share/applications/legend-shot.desktop"
install -m 644 "$PACKAGE_ICON" \
    "$DEB_ROOT/usr/share/icons/hicolor/128x128/apps/legend-shot.png"
sed "s/@VERSION@/$VERSION/g" "$SCRIPT_DIR/control.in" >"$DEB_ROOT/DEBIAN/control"
chmod 644 "$DEB_ROOT/DEBIAN/control"
desktop-file-validate "$DEB_ROOT/usr/share/applications/legend-shot.desktop"

DEB_OUTPUT="$DIST_DIR/legend-shot_${VERSION}_amd64.deb"
rm -f "$DEB_OUTPUT"
dpkg-deb --root-owner-group --build "$DEB_ROOT" "$DEB_OUTPUT"
[[ -s "$DEB_OUTPUT" ]] || fail "DEB package was not created"

APPDIR="$STAGE_DIR/AppDir"
PLUGIN_PATH="$STAGE_DIR/linuxdeploy-plugins"
mkdir -p "$APPDIR" "$PLUGIN_PATH"
ln -s "$GTK_PLUGIN" "$PLUGIN_PATH/linuxdeploy-plugin-gtk.sh"

APPIMAGE_OUTPUT="$DIST_DIR/LegendShot-${VERSION}-x86_64.AppImage"
rm -f "$APPIMAGE_OUTPUT"
PATH="$PLUGIN_PATH:$PATH" \
ARCH=x86_64 \
APPIMAGE_EXTRACT_AND_RUN=1 \
LDAI_OUTPUT="$APPIMAGE_OUTPUT" \
"$LINUXDEPLOY_BIN" \
    --appdir "$APPDIR" \
    --executable "$BINARY" \
    --executable "$(command -v xclip)" \
    --desktop-file "$SCRIPT_DIR/legend-shot.desktop" \
    --icon-file "$PACKAGE_ICON" \
    --custom-apprun "$SCRIPT_DIR/AppRun" \
    --plugin gtk \
    --output appimage

[[ -s "$APPIMAGE_OUTPUT" && -x "$APPIMAGE_OUTPUT" ]] \
    || fail "AppImage was not created or is not executable"

EXTRACT_DIR="$STAGE_DIR/appimage-extract"
mkdir -p "$EXTRACT_DIR"
(cd "$EXTRACT_DIR" && "$APPIMAGE_OUTPUT" --appimage-extract >/dev/null)
EXTRACT_ROOT="$EXTRACT_DIR/squashfs-root"
[[ -x "$EXTRACT_ROOT/AppRun" ]] || fail "AppImage is missing executable AppRun"
[[ -x "$EXTRACT_ROOT/usr/bin/legend_shot" ]] \
    || fail "AppImage is missing legend_shot"
find "$EXTRACT_ROOT/usr/bin" -type f -name xclip -perm -u+x | grep -q . \
    || fail "AppImage is missing xclip"
find "$EXTRACT_ROOT" -maxdepth 1 \( -type f -o -type l \) -name '*.desktop' | grep -q . \
    || fail "AppImage is missing its root desktop file"

printf 'Linux packaging completed for Legend Shot %s (X11 only).\n' "$VERSION"
printf 'DEB:      %s\n' "$DEB_OUTPUT"
printf 'AppImage: %s\n' "$APPIMAGE_OUTPUT"
