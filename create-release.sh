#!/bin/bash
# 本地运行此脚本创建 release 并上传到 Gitee
# 需要设置 GITEE_TOKEN 环境变量
# 获取 token: https://gitee.com/profile/personal_access_tokens

set -e

VERSION="0.1.0"
OWNER="andnnl"
REPO="legend_shot"

if [ -z "$GITEE_TOKEN" ]; then
    echo "请设置 GITEE_TOKEN 环境变量"
    echo "获取 token: https://gitee.com/profile/personal_access_tokens"
    exit 1
fi

echo "=== 创建 Release v${VERSION} ==="

# 创建 release
RESPONSE=$(curl -s -X POST \
    "https://gitee.com/api/v5/repos/${OWNER}/${REPO}/releases" \
    -H "Content-Type: application/json" \
    -d "{
        \"access_token\": \"${GITEE_TOKEN}\",
        \"tag_name\": \"v${VERSION}\",
        \"name\": \"v${VERSION}\",
        \"body\": \"## Legend Shot v${VERSION}\\n\\n### 功能\\n- 全屏截图选区\\n- 标注工具(画笔/矩形/箭头/文字/马赛克/序号)\\n- 保存到文件\\n- 复制到剪贴板\\n- 中文支持\\n- 配置持久化\\n\\n### 安装\\n```bash\\nchmod +x legend_shot\\nsudo mv legend_shot /usr/local/bin/\\n```\",
        \"draft\": false,
        \"prerelease\": false
    }")

RELEASE_ID=$(echo $RESPONSE | python3 -c "import sys, json; print(json.load(sys.stdin).get('id', ''))")

if [ -z "$RELEASE_ID" ]; then
    echo "创建 release 失败: $RESPONSE"
    exit 1
fi

echo "Release ID: $RELEASE_ID"

# 上传二进制文件
echo "上传 legend_shot 二进制..."
curl -s -X POST \
    "https://gitee.com/api/v5/repos/${OWNER}/${REPO}/releases/${RELEASE_ID}/attach_files" \
    -H "Content-Type: multipart/form-data" \
    -F "access_token=${GITEE_TOKEN}" \
    -F "file=@target/release/legend_shot;filename=legend_shot-${VERSION}-x86_64"

echo ""
echo "=== Release 创建完成 ==="
echo "访问: https://gitee.com/${OWNER}/${REPO}/releases/tag/v${VERSION}"