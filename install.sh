#!/bin/bash

# Legend Shot 截图工具打包安装脚本
# 用法: ./install.sh

echo "========================================="
echo "  Legend Shot 截图工具 - 打包安装脚本"
echo "========================================="

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# 项目目录
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_NAME="legend_shot"
INSTALL_DIR="/usr/local/bin"

# 步骤1: 编译
echo -e "${YELLOW}[1/4] 编译项目...${NC}"
cd "$PROJECT_DIR"
cargo build --release 2>&1 | grep -E "^error|^Compiling legend_shot|Finished" || true

if [ ! -f "target/release/$BINARY_NAME" ]; then
    echo -e "${RED}编译失败！${NC}"
    exit 1
fi
echo -e "${GREEN}编译成功！${NC}"

# 步骤2: 显示二进制信息
echo -e "${YELLOW}[2/4] 二进制文件信息:${NC}"
BINARY_PATH="target/release/$BINARY_NAME"
BINARY_SIZE=$(du -h "$BINARY_PATH" | cut -f1)
echo "  路径: $PROJECT_DIR/$BINARY_PATH"
echo "  大小: $BINARY_SIZE"

# 步骤3: 安装到系统
echo -e "${YELLOW}[3/4] 安装到 $INSTALL_DIR...${NC}"
sudo cp -f "$BINARY_PATH" "$INSTALL_DIR/$BINARY_NAME"
sudo chmod +x "$INSTALL_DIR/$BINARY_NAME"
echo -e "${GREEN}安装成功！${NC}"

# 步骤4: 验证安装
echo -e "${YELLOW}[4/4] 验证安装...${NC}"
if command -v $BINARY_NAME &> /dev/null; then
    INSTALLED_PATH=$(which $BINARY_NAME)
    echo -e "${GREEN}已安装到: $INSTALLED_PATH${NC}"
else
    echo -e "${RED}安装验证失败，请检查 PATH 环境变量${NC}"
fi

echo ""
echo "========================================="
echo -e "${GREEN}  安装完成！${NC}"
echo "========================================="
echo ""
echo "使用方法:"
echo "  在终端运行: $BINARY_NAME"
echo "  或配置快捷键调用"
echo ""
echo "功能说明:"
echo "  - 框选区域后双击: 复制截图到剪贴板"
echo "  - 框选区域后点击工具栏: 添加标注/保存/复制"
echo "  - 按 ESC 键退出"
echo ""
