#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 安装所需的编译目标
setup_targets() {
    echo "安装编译目标..."
    rustup target add x86_64-unknown-linux-gnu
    rustup target add aarch64-unknown-linux-gnu
    rustup target add x86_64-pc-windows-msvc
    rustup target add aarch64-apple-darwin
    rustup target add x86_64-apple-darwin
}

pkg_config_exists() {
    local pkg_config_libdir="$1"
    local package="$2"
    if [ -n "$pkg_config_libdir" ]; then
        PKG_CONFIG_LIBDIR="$pkg_config_libdir" pkg-config --exists "$package"
    else
        pkg-config --exists "$package"
    fi
}

check_linux_tray_dependencies() {
    local pkg_config_libdir="$1"
    if ! pkg_config_exists "$pkg_config_libdir" gtk+-3.0; then
        echo "缺少 GTK3 开发包：Ubuntu 请安装 libgtk-3-dev"
        return 1
    fi
    if ! pkg_config_exists "$pkg_config_libdir" ayatana-appindicator3-0.1 \
        && ! pkg_config_exists "$pkg_config_libdir" appindicator3-0.1; then
        echo "缺少 AppIndicator 开发包：Ubuntu 请安装 libayatana-appindicator3-dev 或 libappindicator3-dev"
        return 1
    fi
}

# 编译Linux AMD64版本
build_linux_amd64() {
    echo "编译Linux AMD64版本..."
    check_linux_tray_dependencies "" || return 1
    cargo build --release --target=x86_64-unknown-linux-gnu
    echo "输出文件: target/x86_64-unknown-linux-gnu/release/legend_shot"
}

# 编译Linux ARM64版本
build_linux_arm64() {
    echo "编译Linux ARM64版本..."
    local pkg_config_libdir=""
    if [ "$(uname -m)" != "aarch64" ]; then
        if ! command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
            echo "缺少 ARM64 Linux 交叉编译器：Ubuntu 请安装 gcc-aarch64-linux-gnu"
            return 1
        fi
        pkg_config_libdir="/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig"
    fi
    check_linux_tray_dependencies "$pkg_config_libdir" || return 1
    cargo build --release --target=aarch64-unknown-linux-gnu
    echo "输出文件: target/aarch64-unknown-linux-gnu/release/legend_shot"
}

# 编译Windows版本
build_windows() {
    echo "编译Windows版本..."
    cargo build --release --target=x86_64-pc-windows-msvc
    echo "输出文件: target/x86_64-pc-windows-msvc/release/legend_shot.exe"
}

# 编译macOS ARM64版本
build_macos_arm64() {
    echo "编译macOS ARM64版本..."
    cargo build --release --target=aarch64-apple-darwin
    echo "输出文件: target/aarch64-apple-darwin/release/legend_shot"
}

# 编译macOS Intel版本
build_macos_intel() {
    echo "编译macOS Intel版本..."
    cargo build --release --target=x86_64-apple-darwin
    echo "输出文件: target/x86_64-apple-darwin/release/legend_shot"
}

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

print_usage() {
    echo "用法: $0 {setup|linux-amd64|linux-arm64|windows|macos-arm64|macos-intel|all|package-windows|package-linux|package-macos-arm64|package-macos-intel}"
}

# 编译所有平台
build_all() {
    setup_targets
    build_linux_amd64
    build_linux_arm64
    build_windows
    build_macos_arm64
    build_macos_intel
    echo "所有平台编译完成"
}

# 默认编译所有平台
if [ $# -eq 0 ]; then
    echo "默认编译所有平台..."
    build_all
    exit 0
fi

# 主菜单
case "$1" in
    "setup")
        setup_targets
        ;;
    "linux-amd64")
        build_linux_amd64
        ;;
    "linux-arm64")
        build_linux_arm64
        ;;
    "windows")
        build_windows
        ;;
    "macos-arm64")
        build_macos_arm64
        ;;
    "macos-intel")
        build_macos_intel
        ;;
    "all")
        build_all
        ;;
    "package-windows")
        package_windows
        ;;
    "package-linux")
        package_linux
        ;;
    "package-macos-arm64")
        package_macos_arm64
        ;;
    "package-macos-intel")
        package_macos_intel
        ;;
    "help"|"-h"|"--help")
        print_usage
        ;;
    *)
        print_usage
        exit 1
        ;;
esac
