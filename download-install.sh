#!/bin/bash
set -e

VERSION="0.1.0"
BINARY_NAME="legend_shot-${VERSION}-x86_64"
INSTALL_DIR="/usr/local/bin"
DOWNLOAD_URL="https://gitee.com/andnnl/legend_shot/releases/download/v${VERSION}/${BINARY_NAME}"

echo "=== Legend Shot 安装脚本 v${VERSION} ==="
echo

# 检查是否为 root
if [ "$EUID" -ne 0 ]; then
    echo "需要 root 权限，请使用: sudo ./download-install.sh"
    exit 1
fi

# 检测系统架构
ARCH=$(uname -m)
if [ "$ARCH" != "x86_64" ]; then
    echo "警告: 当前架构 ${ARCH}，此版本仅支持 x86_64"
    read -p "是否继续? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# 检查依赖
echo ">>> 检查依赖..."
MISSING_DEPS=""

# 检查字体
if [ ! -f "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc" ]; then
    if command -v apt-get &> /dev/null; then
        MISSING_DEPS="${MISSING_DEPS} fonts-noto-cjk"
    elif command -v dnf &> /dev/null; then
        MISSING_DEPS="${MISSING_DEPS} google-noto-sans-cjk-fonts"
    elif command -v pacman &> /dev/null; then
        MISSING_DEPS="${MISSING_DEPS} noto-fonts-cjk"
    fi
fi

# 检查 xclip (剪贴板支持)
if ! command -v xclip &> /dev/null; then
    if command -v apt-get &> /dev/null; then
        MISSING_DEPS="${MISSING_DEPS} xclip"
    elif command -v dnf &> /dev/null; then
        MISSING_DEPS="${MISSING_DEPS} xclip"
    elif command -v pacman &> /dev/null; then
        MISSING_DEPS="${MISSING_DEPS} xclip"
    fi
fi

if [ -n "$MISSING_DEPS" ]; then
    echo "缺少依赖: ${MISSING_DEPS}"
    read -p "是否安装? [Y/n] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        if command -v apt-get &> /dev/null; then
            apt-get update && apt-get install -y ${MISSING_DEPS}
        elif command -v dnf &> /dev/null; then
            dnf install -y ${MISSING_DEPS}
        elif command -v pacman &> /dev/null; then
            pacman -S --noconfirm ${MISSING_DEPS}
        fi
    fi
fi

# 下载
echo ">>> 下载 legend_shot v${VERSION}..."
TEMP_FILE="/tmp/${BINARY_NAME}"

if command -v wget &> /dev/null; then
    wget -q --show-progress "${DOWNLOAD_URL}" -O "${TEMP_FILE}"
elif command -v curl &> /dev/null; then
    curl -L --progress-bar "${DOWNLOAD_URL}" -o "${TEMP_FILE}"
else
    echo "错误: 需要 wget 或 curl"
    exit 1
fi

# 安装
echo ">>> 安装到 ${INSTALL_DIR}..."
chmod +x "${TEMP_FILE}"
mv "${TEMP_FILE}" "${INSTALL_DIR}/legend_shot"

# 创建配置目录
CONFIG_DIR="/root/.config/legend_shot"
mkdir -p "${CONFIG_DIR}"

echo
echo "✅ 安装完成!"
echo
echo "使用方法:"
echo "  legend_shot          # 启动截图"
echo "  legend_shot --help   # 查看帮助"
echo
echo "配置快捷键 (GNOME):"
echo "  设置 -> 键盘 -> 自定义快捷键 -> 添加"
echo "  名称: Legend Shot"
echo "  命令: legend_shot"
echo "  快捷键: 自定义 (如 Ctrl+Alt+A)"