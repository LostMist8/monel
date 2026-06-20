#!/bin/bash

# Monel Gateway 启动脚本

echo "🚀 启动 Monel Gateway..."

# 检查配置文件
if [ ! -f "config.yaml" ]; then
    echo "❌ 错误: 找不到 config.yaml 配置文件"
    exit 1
fi

# 检查 page 目录
if [ ! -d "page" ]; then
    echo "❌ 错误: 找不到 page 目录"
    exit 1
fi

# 启动服务器
echo "📦 编译项目..."
cargo build --release

if [ $? -eq 0 ]; then
    echo "✅ 编译成功"
    echo ""
    echo "🌐 启动服务器..."
    ./target/release/gateway server
else
    echo "❌ 编译失败"
    exit 1
fi
