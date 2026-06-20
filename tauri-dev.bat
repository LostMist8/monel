@echo off
echo.
echo Monel Gateway - Tauri Desktop App
echo =====================================
echo.

REM Check if Rust is installed
where rustc >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] Rust not found
    echo.
    echo Please install Rust first:
    echo   Visit https://rustup.rs/
    echo.
    pause
    exit /b 1
)

REM Check src-tauri directory
if not exist "src-tauri" (
    echo [ERROR] src-tauri directory not found
    echo Please run this script from project root directory
    pause
    exit /b 1
)

echo [OK] Environment check passed
echo.
echo Compiling Tauri application...
echo (First compile may take 5-10 minutes)
echo.

cd src-tauri

REM Development mode
cargo tauri dev

if %errorlevel% neq 0 (
    echo.
    echo [ERROR] Failed to start application
    echo.
    echo Common issues:
    echo   1. Missing system dependencies
    echo   2. Port 7890 already in use
    echo   3. Compilation errors
    echo.
    echo Check error messages above for details
    pause
    exit /b 1
)
