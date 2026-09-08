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

case "${1:-all}" in
    dispatcher) test_dispatcher ;;
    all) test_dispatcher ;;
    *) fail "unknown test group: ${1:-}" ;;
esac

printf 'Packaging contract tests passed: %s\n' "${1:-all}"
