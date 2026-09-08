# Local Cross-Platform Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add repeatable native local packaging for Windows portable/Setup EXE, Linux DEB/AppImage, and separate macOS ARM64/Intel DMGs.

**Architecture:** Keep one root `build.sh` dispatcher and isolate platform behavior under `packaging/<platform>/`. Each platform script resolves the repository root, reads the version from `cargo metadata`, validates tools without installing them, builds one native target, stages only its own files under `dist/`, and optionally signs when complete credentials are present.

**Tech Stack:** Bash, PowerShell 5+, Cargo/rustup, NSIS, ImageMagick, dpkg-deb, linuxdeploy plus GTK plugin, macOS `sips`/`iconutil`/`codesign`/`hdiutil`/`notarytool`, static shell contract tests.

**Spec:** `docs/superpowers/specs/2026-09-08-local-cross-platform-packaging-design.md`

## Global Constraints

- Build Windows on Windows, Linux packages on Linux x86_64, and both macOS targets on macOS; do not pretend one host can emit all desktop packages reliably.
- Produce only Windows x86_64 portable ZIP and Setup EXE, Linux x86_64 DEB and AppImage, and separate macOS ARM64 and x86_64 DMGs.
- Never install tools, invoke `sudo`/`winget`, or download linuxdeploy from packaging scripts.
- Read package name/version with `cargo metadata --no-deps --format-version 1`; build with `cargo build --release --locked --target <target>`.
- Default to unsigned test packages; sign only when the complete platform-specific environment-variable set is present.
- Use `packaging/assets/legend-shot-1024.png` when present and otherwise warn and fall back to `src/icon/tray-focus-128.png`.
- Keep every staging directory below `dist/packaging-<platform>-<arch>` and never clear all of `dist/`.
- Linux remains X11-only; AppImage must bundle xclip, enable the GTK plugin, and install a custom AppRun that prepends AppDir `usr/bin` to PATH.
- Preserve every pre-existing unrelated worktree change. Before editing README files, inspect their current diff and merge the packaging section without replacing concurrent edits.
- Use `apply_patch` for repository edits and stage only files owned by the current task in each commit.

## File Map

- Modify `build.sh`: cross-platform package command dispatcher and corrected binary target/output descriptions.
- Modify `.gitignore`: ignore generated `dist/` package artifacts and staging trees.
- Create `packaging/tests/test_packaging.sh`: host-independent static contract and syntax tests.
- Create `packaging/windows/package.ps1`: Windows build, portable ZIP, NSIS, and optional Authenticode orchestration.
- Create `packaging/windows/installer.nsi`: per-user installer/uninstaller definition.
- Create `packaging/linux/package.sh`: Linux preflight, DEB staging, AppImage staging, and output verification.
- Create `packaging/linux/AppRun`: AppImage runtime PATH wrapper.
- Create `packaging/linux/legend-shot.desktop`: desktop integration metadata.
- Create `packaging/linux/control.in`: Debian metadata template.
- Create `packaging/macos/package.sh`: architecture-specific app bundle, signing, DMG, and notarization.
- Create `packaging/macos/Info.plist.in`: stable bundle metadata template.
- Modify `README.md`: concise English local packaging guide.
- Modify `README.zh-CN.md`: detailed Chinese local packaging guide.

---

### Task 1: Packaging contract test harness and root dispatcher

**Files:**
- Create: `packaging/tests/test_packaging.sh`
- Modify: `build.sh`
- Modify: `.gitignore`

**Interfaces:**
- Consumes: existing `build.sh <command>` CLI.
- Produces: `build.sh --help`, `package-windows`, `package-linux`, `package-macos-arm64`, and `package-macos-intel`; test groups selected as `bash packaging/tests/test_packaging.sh <group>`.

- [ ] **Step 1: Write the failing dispatcher contract test**

Create `packaging/tests/test_packaging.sh` with strict mode, repository-root resolution, and these helpers:

```bash
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
    assert_contains "build.sh" 'packaging/macos/package.sh arm64'
    assert_contains "build.sh" 'packaging/macos/package.sh x86_64'
    assert_contains ".gitignore" '/dist/'
}

case "${1:-all}" in
    dispatcher) test_dispatcher ;;
    all) test_dispatcher ;;
    *) fail "unknown test group: ${1:-}" ;;
esac

printf 'Packaging contract tests passed: %s\n' "${1:-all}"
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
bash packaging/tests/test_packaging.sh dispatcher
```

Expected: FAIL because `build.sh --help` and the four package dispatchers do not exist.

- [ ] **Step 3: Implement the dispatcher minimally**

At the top of `build.sh`, add a stable root:

```bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
```

Correct the existing Windows target to `x86_64-pc-windows-msvc` and every documented output basename from `screenshot*` to Cargo's real `legend_shot` basename. Add these functions:

```bash
package_windows() {
    if command -v powershell.exe >/dev/null 2>&1; then
        powershell.exe -NoProfile -ExecutionPolicy Bypass \
            -File "$SCRIPT_DIR/packaging/windows/package.ps1"
    elif command -v pwsh >/dev/null 2>&1; then
        pwsh -NoProfile -File "$SCRIPT_DIR/packaging/windows/package.ps1"
    else
        echo "Windows 打包需要在 Windows 上安装 PowerShell 后执行。" >&2
        return 1
    fi
}

package_linux() {
    bash "$SCRIPT_DIR/packaging/linux/package.sh"
}

package_macos_arm64() {
    bash "$SCRIPT_DIR/packaging/macos/package.sh" arm64
}

package_macos_intel() {
    bash "$SCRIPT_DIR/packaging/macos/package.sh" x86_64
}
```

Extend the `case` statement with the four commands and accept `help|-h|--help`. The usage string must list every old and new command. Do not make `all` invoke packaging commands because they require different native hosts.

Append this root-anchored generated-artifact rule to `.gitignore`:

```gitignore
/dist/
```

- [ ] **Step 4: Run dispatcher tests and shell syntax checks**

Run:

```bash
bash -n build.sh
bash -n packaging/tests/test_packaging.sh
bash packaging/tests/test_packaging.sh dispatcher
```

Expected: all exit 0.

- [ ] **Step 5: Commit only dispatcher-owned files**

```bash
git add .gitignore build.sh packaging/tests/test_packaging.sh
git commit -m "build: add native packaging entry points"
```

---

### Task 2: Windows portable and NSIS Setup EXE

**Files:**
- Create: `packaging/windows/package.ps1`
- Create: `packaging/windows/installer.nsi`
- Modify: `packaging/tests/test_packaging.sh`

**Interfaces:**
- Consumes: optional `NSIS_PATH`, `WINDOWS_CERT_PFX`, `WINDOWS_CERT_PASSWORD`, `WINDOWS_TIMESTAMP_URL`; required Cargo MSVC target, `magick`, and NSIS.
- Produces: `dist/LegendShot-<version>-Portable-x64.zip` and `dist/LegendShot-<version>-Setup-x64.exe`; supports `package.ps1 -CheckOnly`.

- [ ] **Step 1: Add failing Windows static contracts**

Add this function before the test script's `case`:

```bash
test_windows() {
    assert_file "packaging/windows/package.ps1"
    assert_file "packaging/windows/installer.nsi"
    assert_contains "packaging/windows/package.ps1" 'x86_64-pc-windows-msvc'
    assert_contains "packaging/windows/package.ps1" 'cargo metadata --no-deps --format-version 1'
    assert_contains "packaging/windows/package.ps1" 'Compress-Archive'
    assert_contains "packaging/windows/package.ps1" 'WINDOWS_CERT_PFX'
    assert_contains "packaging/windows/package.ps1" 'WINDOWS_TIMESTAMP_URL'
    assert_contains "packaging/windows/package.ps1" 'magick'
    assert_contains "packaging/windows/installer.nsi" 'RequestExecutionLevel user'
    assert_contains "packaging/windows/installer.nsi" '$LOCALAPPDATA\Programs\Legend Shot'
    assert_contains "packaging/windows/installer.nsi" 'WriteUninstaller'
}
```

Add `windows) test_windows ;;` and invoke it from `all`.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
bash packaging/tests/test_packaging.sh windows
```

Expected: FAIL on the missing PowerShell and NSIS files.

- [ ] **Step 3: Create the NSIS installer definition**

Create `installer.nsi` with required compile-time defines and an all-current-user install:

```nsi
Unicode True
!include "MUI2.nsh"

!ifndef VERSION
  !error "VERSION is required"
!endif
!ifndef SOURCE_EXE
  !error "SOURCE_EXE is required"
!endif
!ifndef OUTPUT_EXE
  !error "OUTPUT_EXE is required"
!endif
!ifndef ICON_FILE
  !error "ICON_FILE is required"
!endif

Name "Legend Shot"
OutFile "${OUTPUT_EXE}"
InstallDir "$LOCALAPPDATA\Programs\Legend Shot"
InstallDirRegKey HKCU "Software\LegendShot" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!define MUI_ICON "${ICON_FILE}"
!define MUI_UNICON "${ICON_FILE}"
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetOutPath "$INSTDIR"
  File /oname=LegendShot.exe "${SOURCE_EXE}"
  CreateDirectory "$SMPROGRAMS\Legend Shot"
  CreateShortcut "$SMPROGRAMS\Legend Shot\Legend Shot.lnk" "$INSTDIR\LegendShot.exe"
  CreateShortcut "$DESKTOP\Legend Shot.lnk" "$INSTDIR\LegendShot.exe"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\LegendShot" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LegendShot" "DisplayName" "Legend Shot"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LegendShot" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LegendShot" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LegendShot" "UninstallString" '"$INSTDIR\Uninstall.exe"'
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\LegendShot.exe"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$DESKTOP\Legend Shot.lnk"
  Delete "$SMPROGRAMS\Legend Shot\Legend Shot.lnk"
  RMDir "$SMPROGRAMS\Legend Shot"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LegendShot"
  DeleteRegKey HKCU "Software\LegendShot"
SectionEnd
```

- [ ] **Step 4: Create the Windows orchestration script**

Implement `package.ps1` with this control flow:

```powershell
param([switch]$CheckOnly)
$ErrorActionPreference = "Stop"
$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$Target = "x86_64-pc-windows-msvc"
$DistDir = Join-Path $ProjectRoot "dist"
$StageDir = Join-Path $DistDir "packaging-windows-x64"

if (-not $IsWindows -and $PSVersionTable.PSEdition -eq "Core") {
    throw "Windows packages must be built on Windows."
}

foreach ($Command in @("cargo", "rustup", "magick")) {
    if (-not (Get-Command $Command -ErrorAction SilentlyContinue)) {
        throw "Missing command: $Command. Install it manually, then retry."
    }
}

$InstalledTargets = rustup target list --installed
if ($InstalledTargets -notcontains $Target) {
    throw "Missing Rust target $Target. Run: rustup target add $Target"
}

$MakeNsis = if ($env:NSIS_PATH) {
    if (Test-Path $env:NSIS_PATH -PathType Container) {
        Join-Path $env:NSIS_PATH "makensis.exe"
    } else {
        $env:NSIS_PATH
    }
} else {
    (Get-Command makensis.exe -ErrorAction SilentlyContinue).Source
}
if (-not $MakeNsis -or -not (Test-Path $MakeNsis)) {
    throw "NSIS was not found. Install NSIS or set NSIS_PATH."
}

$Metadata = cargo metadata --manifest-path (Join-Path $ProjectRoot "Cargo.toml") --no-deps --format-version 1 | ConvertFrom-Json
$Package = $Metadata.packages | Where-Object { $_.name -eq "legend_shot" } | Select-Object -First 1
if (-not $Package) { throw "cargo metadata did not return legend_shot" }
$Version = $Package.version

$SignValues = @($env:WINDOWS_CERT_PFX, $env:WINDOWS_CERT_PASSWORD, $env:WINDOWS_TIMESTAMP_URL)
$ConfiguredSignValues = @($SignValues | Where-Object { $_ })
if ($ConfiguredSignValues.Count -ne 0 -and $ConfiguredSignValues.Count -ne 3) {
    throw "Set all or none of WINDOWS_CERT_PFX, WINDOWS_CERT_PASSWORD, WINDOWS_TIMESTAMP_URL."
}
$SigningEnabled = $ConfiguredSignValues.Count -eq 3
if ($SigningEnabled -and -not (Get-Command signtool.exe -ErrorAction SilentlyContinue)) {
    throw "signtool.exe was not found in PATH."
}

if ($CheckOnly) {
    Write-Host "Windows packaging preflight passed for Legend Shot $Version"
    exit 0
}
```

Continue by running Cargo from `$ProjectRoot`, checking `$LASTEXITCODE`, recreating only `$StageDir`, choosing `packaging/assets/legend-shot-1024.png` or the 128 px fallback, and generating `$StageDir\legend-shot.ico`:

```powershell
Push-Location $ProjectRoot
try {
    & cargo build --release --locked --target $Target
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
} finally { Pop-Location }

$Binary = Join-Path $ProjectRoot "target\$Target\release\legend_shot.exe"
if (-not (Test-Path $Binary) -or (Get-Item $Binary).Length -eq 0) { throw "Release binary is missing" }

New-Item -ItemType Directory -Force $DistDir | Out-Null
if (Test-Path $StageDir) { Remove-Item -Recurse -Force $StageDir }
New-Item -ItemType Directory -Force $StageDir | Out-Null

$PreferredIcon = Join-Path $ProjectRoot "packaging\assets\legend-shot-1024.png"
$FallbackIcon = Join-Path $ProjectRoot "src\icon\tray-focus-128.png"
$IconSource = if (Test-Path $PreferredIcon) { $PreferredIcon } else {
    Write-Warning "Using the 128x128 tray icon; replace it before public release."
    $FallbackIcon
}
$IconFile = Join-Path $StageDir "legend-shot.ico"
& magick $IconSource -define "icon:auto-resize=256,128,64,48,32,16" $IconFile
if ($LASTEXITCODE -ne 0) { throw "ICO generation failed" }
```

When signing is enabled, sign `$Binary` before copying it. Copy it as `LegendShot.exe`, create the portable ZIP, invoke NSIS with `/DVERSION`, `/DSOURCE_EXE`, `/DOUTPUT_EXE`, and `/DICON_FILE`, then sign the Setup EXE. Every native command must check `$LASTEXITCODE`. Print version, two output paths, and signed/unsigned state at the end without printing the password.

- [ ] **Step 5: Verify Windows contracts**

Run on the current host:

```bash
bash packaging/tests/test_packaging.sh windows
```

On Windows also run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File packaging\windows\package.ps1 -CheckOnly
powershell.exe -NoProfile -ExecutionPolicy Bypass -File packaging\windows\package.ps1
```

Expected: static tests pass everywhere; Windows produces both files and reports unsigned when signing variables are absent.

- [ ] **Step 6: Commit Windows packaging**

```bash
git add packaging/windows packaging/tests/test_packaging.sh
git commit -m "build: add Windows portable and NSIS packages"
```

---

### Task 3: Linux DEB and AppImage

**Files:**
- Create: `packaging/linux/package.sh`
- Create: `packaging/linux/AppRun`
- Create: `packaging/linux/legend-shot.desktop`
- Create: `packaging/linux/control.in`
- Modify: `packaging/tests/test_packaging.sh`

**Interfaces:**
- Consumes: required `LINUXDEPLOY` or PATH linuxdeploy; required `LINUXDEPLOY_PLUGIN_GTK` or discoverable GTK plugin; Linux x86_64 build dependencies and xclip.
- Produces: `dist/legend-shot_<version>_amd64.deb`, `dist/LegendShot-<version>-x86_64.AppImage`; supports `package.sh --check`.

- [ ] **Step 1: Add failing Linux contracts**

Add `test_linux` and wire it into the test script:

```bash
test_linux() {
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
```

- [ ] **Step 2: Run and verify RED**

```bash
bash packaging/tests/test_packaging.sh linux
```

Expected: FAIL because the Linux packaging files do not exist.

- [ ] **Step 3: Create runtime and metadata templates**

Create `AppRun`:

```bash
#!/bin/sh
set -eu
APPDIR_PATH="${APPDIR:-$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)}"
PATH="$APPDIR_PATH/usr/bin:$PATH"
export PATH
exec "$APPDIR_PATH/usr/bin/legend_shot" "$@"
```

Create `legend-shot.desktop`:

```ini
[Desktop Entry]
Type=Application
Name=Legend Shot
Comment=Screenshot and annotation tool
Exec=legend_shot
Icon=legend-shot
Terminal=false
Categories=Graphics;Utility;
StartupNotify=false
```

Create `control.in`:

```text
Package: legend-shot
Version: @VERSION@
Section: graphics
Priority: optional
Architecture: amd64
Maintainer: Legend Shot
Depends: libgtk-3-0, libayatana-appindicator3-1 | libappindicator3-1, libxdo3, xclip
Description: Screenshot and annotation tool
 Legend Shot provides local screenshot capture, annotation, clipboard,
 OCR, and experimental multi-monitor capture under Linux X11.
```

- [ ] **Step 4: Implement Linux preflight and metadata resolution**

Create `package.sh` with strict mode and these exact public behaviors:

```bash
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DIST_DIR="$PROJECT_ROOT/dist"
TARGET="x86_64-unknown-linux-gnu"
CHECK_ONLY=false
[[ "${1:-}" == "--check" ]] && CHECK_ONLY=true

fail() { printf 'Linux packaging error: %s\n' "$1" >&2; exit 1; }
need_command() { command -v "$1" >/dev/null 2>&1 || fail "missing command '$1'"; }

[[ "$(uname -s)" == "Linux" ]] || fail "Linux packages must be built on Linux"
[[ "$(uname -m)" == "x86_64" ]] || fail "this script currently packages x86_64 only"
for command in cargo rustup python3 pkg-config dpkg-deb desktop-file-validate xclip; do
    need_command "$command"
done
rustup target list --installed | grep -Fxq "$TARGET" \
    || fail "run: rustup target add $TARGET"
pkg-config --exists gtk+-3.0 || fail "install the GTK3 development package"
pkg-config --exists xdo || fail "install the libxdo development package"
if ! pkg-config --exists ayatana-appindicator3-0.1 \
    && ! pkg-config --exists appindicator3-0.1; then
    fail "install an AppIndicator development package"
fi
```

Resolve linuxdeploy from `LINUXDEPLOY` or PATH. Resolve the GTK plugin from `LINUXDEPLOY_PLUGIN_GTK`, PATH, or the directory containing linuxdeploy. Validate both are executable. Read version with Cargo metadata piped into `python3 -c`, and print a successful preflight/exit when `--check` is set.

When `LINUXDEPLOY_PLUGIN_GTK` points outside linuxdeploy's discovery locations, create a staging symlink named `linuxdeploy-plugin-gtk.sh` to it and prepend that staging directory to PATH for the linuxdeploy command. Do not copy or download the plugin.

- [ ] **Step 5: Implement DEB and AppImage staging**

Build the target with Cargo. Recreate only:

```text
dist/packaging-linux-x86_64/deb-root
dist/packaging-linux-x86_64/AppDir
```

Install the binary, Desktop file, and icon into the DEB tree with modes 755/644. Replace `@VERSION@` in `control.in`, validate the Desktop file, and call:

```bash
dpkg-deb --root-owner-group --build "$DEB_ROOT" "$DEB_OUTPUT"
```

For AppImage, call linuxdeploy with main binary, `$(command -v xclip)`, Desktop file, icon, GTK plugin, and custom AppRun:

```bash
LDAI_OUTPUT="$APPIMAGE_OUTPUT" "$LINUXDEPLOY_BIN" \
    --appdir "$APPDIR" \
    --executable "$BINARY" \
    --executable "$(command -v xclip)" \
    --desktop-file "$SCRIPT_DIR/legend-shot.desktop" \
    --icon-file "$ICON_SOURCE" \
    --custom-apprun "$SCRIPT_DIR/AppRun" \
    --plugin gtk \
    --output appimage
```

Verify both outputs are non-empty and executable as appropriate. Extract the AppImage to the platform staging directory with `--appimage-extract`, then assert `usr/bin/legend_shot`, an xclip executable, the root Desktop file, and executable `AppRun` exist. Print the two paths and the X11-only warning.

- [ ] **Step 6: Verify Linux contracts**

On the current host:

```bash
bash packaging/tests/test_packaging.sh linux
```

On Linux x86_64:

```bash
packaging/linux/package.sh --check
packaging/linux/package.sh
dpkg-deb --info dist/legend-shot_0.1.0_amd64.deb
dist/LegendShot-0.1.0-x86_64.AppImage --appimage-extract-and-run
```

Expected: static tests pass; native preflight passes without modifying the system; both packages are created.

- [ ] **Step 7: Commit Linux packaging**

```bash
git add packaging/linux packaging/tests/test_packaging.sh
git commit -m "build: add Linux DEB and AppImage packages"
```

---

### Task 4: Separate macOS ARM64 and Intel DMGs

**Files:**
- Create: `packaging/macos/package.sh`
- Create: `packaging/macos/Info.plist.in`
- Modify: `packaging/tests/test_packaging.sh`

**Interfaces:**
- Consumes: positional architecture `arm64|x86_64`; optional `MACOS_SIGN_IDENTITY`, `MACOS_NOTARY_PROFILE`, `MACOS_BUILD_NUMBER`, and `MACOS_MIN_VERSION`.
- Produces: `dist/LegendShot-<version>-macos-arm64.dmg` or `dist/LegendShot-<version>-macos-x86_64.dmg`; supports `package.sh <arch> --check`.

- [ ] **Step 1: Add failing macOS contracts**

```bash
test_macos() {
    assert_file "packaging/macos/package.sh"
    assert_file "packaging/macos/Info.plist.in"
    bash -n "$PROJECT_ROOT/packaging/macos/package.sh"
    assert_contains "packaging/macos/package.sh" 'aarch64-apple-darwin'
    assert_contains "packaging/macos/package.sh" 'x86_64-apple-darwin'
    assert_contains "packaging/macos/package.sh" 'MACOS_SIGN_IDENTITY'
    assert_contains "packaging/macos/package.sh" 'MACOS_NOTARY_PROFILE'
    assert_contains "packaging/macos/package.sh" 'notarytool submit'
    assert_contains "packaging/macos/package.sh" 'stapler staple'
    assert_contains "packaging/macos/Info.plist.in" 'com.legend.legend-shot'
    assert_contains "packaging/macos/Info.plist.in" '<key>LSUIElement</key>'
}
```

- [ ] **Step 2: Run and verify RED**

```bash
bash packaging/tests/test_packaging.sh macos
```

Expected: FAIL because macOS packaging files are missing.

- [ ] **Step 3: Create the plist template**

Create valid XML `Info.plist.in` with substitutions `@VERSION@`, `@BUILD_NUMBER@`, and `@MIN_VERSION@`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>zh_CN</string>
  <key>CFBundleDisplayName</key><string>Legend Shot</string>
  <key>CFBundleExecutable</key><string>legend_shot</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundleIdentifier</key><string>com.legend.legend-shot</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>Legend Shot</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>@VERSION@</string>
  <key>CFBundleVersion</key><string>@BUILD_NUMBER@</string>
  <key>LSMinimumSystemVersion</key><string>@MIN_VERSION@</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
```

- [ ] **Step 4: Implement macOS preflight and architecture mapping**

Create `package.sh` with strict mode, reject non-macOS hosts, accept only `arm64` or `x86_64`, and map them exactly:

```bash
case "$ARCH" in
    arm64) TARGET="aarch64-apple-darwin" ;;
    x86_64) TARGET="x86_64-apple-darwin" ;;
    *) fail "usage: package.sh {arm64|x86_64} [--check]" ;;
esac
```

Check `cargo`, `rustup`, `python3`, `xcode-select`, `sips`, `iconutil`, `codesign`, `hdiutil`, `plutil`, `file`, and `ditto`. Verify `xcode-select -p`, the Rust target, and complete signing logic:

- Empty `MACOS_SIGN_IDENTITY` means unsigned/ad-hoc test package.
- Non-empty `MACOS_NOTARY_PROFILE` with empty identity is an error.
- Default `MACOS_BUILD_NUMBER=1` and `MACOS_MIN_VERSION=11.0`.
- `--check` prints version, architecture, target, signing state, and notarization state without building.

- [ ] **Step 5: Implement icon, app bundle, signing, and DMG**

Build the selected target with `MACOSX_DEPLOYMENT_TARGET="$MACOS_MIN_VERSION"` so the binary deployment target matches `LSMinimumSystemVersion`. Recreate only `dist/packaging-macos-<arch>`. Generate the ten standard iconset files with `sips`, convert them with `iconutil`, and warn when using the 128 px fallback. Create:

```text
dist/packaging-macos-<arch>/Legend Shot.app/Contents/MacOS/legend_shot
dist/packaging-macos-<arch>/Legend Shot.app/Contents/Resources/AppIcon.icns
dist/packaging-macos-<arch>/Legend Shot.app/Contents/Info.plist
```

Render plist substitutions with Python, run `plutil -lint`, and validate the binary's `file` output contains `arm64` or `x86_64` as requested. Without a Developer ID, apply ad-hoc signing:

```bash
codesign --force --deep --sign - "$APP_BUNDLE"
```

With `MACOS_SIGN_IDENTITY`, run:

```bash
codesign --force --options runtime --timestamp \
    --sign "$MACOS_SIGN_IDENTITY" "$APP_BUNDLE"
codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE"
```

Stage the app with `ditto`, create a relative staging symlink to `/Applications`, and call:

```bash
hdiutil create -volname "Legend Shot" -srcfolder "$DMG_ROOT" \
    -ov -format UDZO "$DMG_OUTPUT"
hdiutil verify "$DMG_OUTPUT"
```

If notarization is enabled, submit with `xcrun notarytool submit "$DMG_OUTPUT" --keychain-profile "$MACOS_NOTARY_PROFILE" --wait`, then run `xcrun stapler staple` and `xcrun stapler validate`. Print output, architecture, signature, and notarization status.

- [ ] **Step 6: Verify macOS contracts and both packages**

```bash
bash packaging/tests/test_packaging.sh macos
packaging/macos/package.sh arm64 --check
packaging/macos/package.sh x86_64 --check
packaging/macos/package.sh arm64
packaging/macos/package.sh x86_64
hdiutil verify dist/LegendShot-0.1.0-macos-arm64.dmg
hdiutil verify dist/LegendShot-0.1.0-macos-x86_64.dmg
```

Expected: both unsigned test DMGs build on the current Mac when both Rust targets compile; each contains only its requested architecture.

- [ ] **Step 7: Commit macOS packaging**

```bash
git add packaging/macos packaging/tests/test_packaging.sh
git commit -m "build: add separate macOS DMG packages"
```

---

### Task 5: User-facing local packaging documentation

**Files:**
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `packaging/tests/test_packaging.sh`

**Interfaces:**
- Consumes: all package commands and environment variables created in Tasks 1-4.
- Produces: discoverable native prerequisites, commands, output table, signing instructions, and release validation matrix.

- [ ] **Step 1: Inspect concurrent README changes before editing**

Run:

```bash
git diff -- README.md README.zh-CN.md
```

Expected: possibly non-empty because another active task owns existing edits. Preserve them and insert packaging documentation into the current “Build/构建” section only.

- [ ] **Step 2: Add failing documentation contracts**

```bash
test_docs() {
    for file in README.md README.zh-CN.md; do
        assert_contains "$file" 'package-windows'
        assert_contains "$file" 'package-linux'
        assert_contains "$file" 'package-macos-arm64'
        assert_contains "$file" 'package-macos-intel'
        assert_contains "$file" 'WINDOWS_CERT_PFX'
        assert_contains "$file" 'MACOS_SIGN_IDENTITY'
        assert_contains "$file" 'MACOS_NOTARY_PROFILE'
        assert_contains "$file" 'LINUXDEPLOY_PLUGIN_GTK'
    done
}
```

- [ ] **Step 3: Run and verify RED**

```bash
bash packaging/tests/test_packaging.sh docs
```

Expected: FAIL because the README files do not yet document package commands and environment variables.

- [ ] **Step 4: Update both README build sections**

Document the exact package matrix:

```text
package-windows       -> Portable-x64.zip + Setup-x64.exe
package-linux         -> amd64.deb + x86_64.AppImage
package-macos-arm64   -> macos-arm64.dmg
package-macos-intel   -> macos-x86_64.dmg
```

For each host, list manual prerequisite installation examples, direct platform-script `--check` calls, unified commands, optional signing variables, and artifact paths. State explicitly:

- Package scripts never install/download dependencies.
- Windows uses MSVC and needs NSIS/ImageMagick; current GUI subsystem caveat remains.
- Linux AppImage needs linuxdeploy plus GTK plugin at user-provided paths, bundles xclip, targets X11, and still requires clean-distribution testing.
- macOS uses a stable `com.legend.legend-shot` bundle ID, generates separate architectures, and public distribution needs Developer ID plus notarization.
- The fallback 128 px icon is suitable only for testing.

- [ ] **Step 5: Verify documentation contracts**

```bash
bash packaging/tests/test_packaging.sh docs
bash packaging/tests/test_packaging.sh all
git diff --check
```

Expected: all pass without altering unrelated README content.

- [ ] **Step 6: Commit docs and their test update only**

```bash
git add README.md README.zh-CN.md packaging/tests/test_packaging.sh
git commit -m "docs: add native packaging guide"
```

If README files still contain unrelated uncommitted hunks, use an interactive or patch-scoped staging method and verify `git diff --cached` contains only packaging documentation before committing.

---

### Task 6: Final verification and platform handoff

**Files:**
- Modify only if verification exposes a packaging-owned defect.

**Interfaces:**
- Consumes: completed packaging files and current application code.
- Produces: verified macOS artifacts where host tooling permits and an explicit Windows/Linux native validation checklist.

- [ ] **Step 1: Run repository-independent packaging checks**

```bash
bash packaging/tests/test_packaging.sh all
bash -n build.sh
bash -n packaging/linux/package.sh
bash -n packaging/linux/AppRun
bash -n packaging/macos/package.sh
git diff --check
```

Expected: all exit 0.

- [ ] **Step 2: Run application verification**

```bash
cargo fmt --all -- --check
cargo test
cargo build
cargo clippy --all-targets -- -D warnings
```

Expected: all Rust checks pass. If unrelated concurrent changes fail, record the exact failing files/tests and do not modify them under this task.

- [ ] **Step 3: Run macOS preflight and native package build**

```bash
packaging/macos/package.sh arm64 --check
packaging/macos/package.sh x86_64 --check
packaging/macos/package.sh arm64
packaging/macos/package.sh x86_64
```

Verify each existing output:

```bash
hdiutil verify dist/LegendShot-0.1.0-macos-arm64.dmg
hdiutil verify dist/LegendShot-0.1.0-macos-x86_64.dmg
```

If x86_64 build fails because a dependency or target is unavailable, keep ARM64 evidence and report the blocker rather than claiming both DMGs succeeded.

- [ ] **Step 4: Inspect the final task-owned diff and worktree**

```bash
git status --short
git log --oneline -8
git diff --check HEAD
```

Expected: no uncommitted packaging-owned files; unrelated pre-existing changes may remain and must be listed separately.

- [ ] **Step 5: Record native follow-up commands in the handoff**

Windows operator must run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File packaging\windows\package.ps1 -CheckOnly
powershell.exe -NoProfile -ExecutionPolicy Bypass -File packaging\windows\package.ps1
```

Linux operator must run:

```bash
packaging/linux/package.sh --check
packaging/linux/package.sh
```

The final report must distinguish static validation from native package execution and must not claim Windows/Linux artifacts were produced on macOS.
