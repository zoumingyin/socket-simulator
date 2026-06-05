#!/bin/bash
# Socket 服务管理平台 - 完整构建脚本
# 请在项目根目录（非沙盒终端）运行此脚本

set -e

echo "========================================="
echo "  Socket 服务管理平台 - 构建脚本"
echo "========================================="
echo ""

# 1. 前端构建
echo "[1/4] 构建前端 (Vite)..."
cd "$(dirname "$0")"
npm run build
echo "✅ 前端构建完成 → dist/"
echo ""

# 2. 后端构建
echo "[2/4] 构建后端 (TypeScript → JS)..."
cd backend
npx tsc
cd ..
echo "✅ 后端构建完成 → backend/dist/"
echo ""

# 3. 确认图标文件存在
echo "[3/4] 检查 Tauri 图标..."
if [ ! -f "src-tauri/icons/icon.png" ]; then
    echo "⚠️  图标文件不存在，正在从 icon.svg 生成..."
    if command -v npx &> /dev/null; then
        npx @tauri-apps/cli@latest icon src-tauri/icon.svg 2>/dev/null || \
        echo "  请手动运行: npx @tauri-apps/cli@latest icon src-tauri/icon.svg"
    fi
fi
echo "✅ 图标检查完成"
echo ""

# 4. 构建 Tauri 应用
echo "[4/4] 构建 Tauri 桌面应用..."
echo "  注意：首次构建会下载 Rust 依赖，可能需要 10-30 分钟"
echo ""
cd src-tauri
cargo build --release
cd ..
echo ""
echo "✅ Tauri 构建完成！"
echo ""
echo "========================================="
echo "  产物位置："
echo "  - 桌面应用: src-tauri/target/release/socket-service-manager.exe"
echo "  - 前端产物: dist/"
echo "  - 后端产物: backend/dist/"
echo "========================================="
echo ""
echo "运行应用："
echo "  cd src-tauri && cargo run --release"
echo ""
