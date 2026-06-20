@echo off
chcp 65001 >nul
echo.
echo 🚀 启动 Monel Gateway...
echo.

REM 检查配置文件
if not exist "config.yaml" (
    echo ❌ 错误: 找不到 config.yaml 配置文件
    pause
    exit /b 1
)

REM 检查 page 目录
if not exist "page" (
    echo ❌ 错误: 找不到 page 目录
    pause
    exit /b 1
)

REM 启动服务器
echo 📦 编译项目...
cargo build --release

if %errorlevel% equ 0 (
    echo ✅ 编译成功
    echo.
    echo 🌐 启动服务器...
    echo.
    echo 访问地址:
    echo   - 欢迎页: http://127.0.0.1:7890/
    echo   - 管理后台: http://127.0.0.1:7890/admin.html
    echo.
    echo 按 Ctrl+C 停止服务器
    echo.
    target\release\gateway.exe server
) else (
    echo ❌ 编译失败
    pause
    exit /b 1
)
