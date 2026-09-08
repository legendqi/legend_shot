#!/usr/bin/env bash
set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$TEST_DIR/../.." && pwd)"

fail() {
    printf 'FAIL: %s\n' "$1" >&2
    exit 1
}

assert_file() {
    [[ -f "$PROJECT_ROOT/$1" ]] || fail "missing file: $1"
}

assert_contains() {
    local file="$1"
    local text="$2"
    grep -Fq -- "$text" "$PROJECT_ROOT/$file" \
        || fail "$file does not contain: $text"
}

test_dispatcher() {
    local output
    output="$(bash "$PROJECT_ROOT/build.sh" --help 2>&1 || true)"
    [[ "$output" == *"package-windows"* ]] || fail "help misses package-windows"
    [[ "$output" == *"package-linux"* ]] || fail "help misses package-linux"
    [[ "$output" == *"package-macos-arm64"* ]] || fail "help misses package-macos-arm64"
    [[ "$output" == *"package-macos-intel"* ]] || fail "help misses package-macos-intel"
    assert_contains "build.sh" 'packaging/windows/package.ps1'
    assert_contains "build.sh" 'packaging/linux/package.sh'
    assert_contains "build.sh" 'packaging/macos/package.sh" arm64'
    assert_contains "build.sh" 'packaging/macos/package.sh" x86_64'
    assert_contains ".gitignore" '/dist/'
}

test_windows() {
    assert_file "packaging/windows/package.ps1"
    assert_file "packaging/windows/installer.nsi"
    assert_contains "packaging/windows/package.ps1" 'x86_64-pc-windows-msvc'
    assert_contains "packaging/windows/package.ps1" 'cargo metadata'
    assert_contains "packaging/windows/package.ps1" '--no-deps'
    assert_contains "packaging/windows/package.ps1" '--format-version 1'
    assert_contains "packaging/windows/package.ps1" 'Compress-Archive'
    assert_contains "packaging/windows/package.ps1" 'WINDOWS_CERT_PFX'
    assert_contains "packaging/windows/package.ps1" 'WINDOWS_TIMESTAMP_URL'
    assert_contains "packaging/windows/package.ps1" 'magick'
    assert_contains "packaging/windows/installer.nsi" 'RequestExecutionLevel user'
    assert_contains "packaging/windows/installer.nsi" '$LOCALAPPDATA\Programs\Legend Shot'
    assert_contains "packaging/windows/installer.nsi" 'WriteUninstaller'
}

test_linux() {
    local file
    for file in package.sh AppRun legend-shot.desktop control.in; do
        assert_file "packaging/linux/$file"
    done
    bash -n "$PROJECT_ROOT/packaging/linux/package.sh"
    bash -n "$PROJECT_ROOT/packaging/linux/AppRun"
    assert_contains "packaging/linux/package.sh" 'x86_64-unknown-linux-gnu'
    assert_contains "packaging/linux/package.sh" 'LINUXDEPLOY_PLUGIN_GTK'
    assert_contains "packaging/linux/package.sh" '--custom-apprun'
    assert_contains "packaging/linux/package.sh" '--plugin gtk'
    assert_contains "packaging/linux/package.sh" 'LDAI_OUTPUT'
    assert_contains "packaging/linux/AppRun" 'usr/bin:$PATH'
    assert_contains "packaging/linux/AppRun" 'usr/bin/legend_shot'
    assert_contains "packaging/linux/control.in" 'xclip'
}

case "${1:-all}" in
    dispatcher) test_dispatcher ;;
    windows) test_windows ;;
    linux) test_linux ;;
    all)
        test_dispatcher
        test_windows
        test_linux
        ;;
    *) fail "unknown test group: ${1:-}" ;;
esac

printf 'Packaging contract tests passed: %s\n' "${1:-all}"
