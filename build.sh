#!/bin/bash

# 安装所需的编译目标
setup_targets() {
    echo "安装编译目标..."
    rustup target add x86_64-unknown-linux-gnu
    rustup target add aarch64-unknown-linux-gnu
    rustup target add x86_64-pc-windows-msvc
    rustup target add aarch64-apple-darwin
    rustup target add x86_64-apple-darwin
}

check_linux_tray_dependencies() {
    if ! pkg-config --exists gtk+-3.0; then
        echo "缺少 GTK3 开发包：Ubuntu 请安装 libgtk-3-dev"
        return 1
    fi
    if ! pkg-config --exists ayatana-appindicator3-0.1 \
        && ! pkg-config --exists appindicator3-0.1; then
        echo "缺少 AppIndicator 开发包：Ubuntu 请安装 libayatana-appindicator3-dev 或 libappindicator3-dev"
        return 1
    fi
}

# 编译Linux AMD64版本
build_linux_amd64() {
    echo "编译Linux AMD64版本..."
    check_linux_tray_dependencies || return 1
    cargo build --release --target=x86_64-unknown-linux-gnu
    echo "输出文件: target/x86_64-unknown-linux-gnu/release/screenshot-linux-amd64"
}

# 编译Linux ARM64版本
build_linux_arm64() {
    echo "编译Linux ARM64版本..."
    check_linux_tray_dependencies || return 1
    cargo build --release --target=aarch64-unknown-linux-gnu
    echo "输出文件: target/aarch64-unknown-linux-gnu/release/screenshot-linux-arm64"
}

# 编译Windows版本
build_windows() {
    echo "编译Windows版本..."
    cargo build --release --target=x86_64-pc-windows-gnu
    echo "输出文件: target/x86_64-pc-windows-gnu/release/screenshot.exe"
}

# 编译macOS ARM64版本
build_macos_arm64() {
    echo "编译macOS ARM64版本..."
    cargo build --release --target=aarch64-apple-darwin
    echo "输出文件: target/aarch64-apple-darwin/release/screenshot-macos-arm64"
}

# 编译macOS Intel版本
build_macos_intel() {
    echo "编译macOS Intel版本..."
    cargo build --release --target=x86_64-apple-darwin
    echo "输出文件: target/x86_64-apple-darwin/release/screenshot-macos-x86_64"
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
    *)
        echo "用法: $0 {setup|linux-amd64|linux-arm64|windows|macos-arm64|macos-intel|all}"
        exit 1
        ;;
esac
