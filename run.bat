@echo off
REM Monel Gateway - Quick Start Script
REM Run from project root directory

where rustc >nul 2>nul
if %errorlevel% neq 0 (
    echo ERROR: Rust not installed. Visit https://rustup.rs/
    pause
    exit /b 1
)

if not exist "src-tauri" (
    echo ERROR: Run this from project root directory
    pause
    exit /b 1
)

echo Starting Monel Gateway...
cd src-tauri
cargo tauri dev
