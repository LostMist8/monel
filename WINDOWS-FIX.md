# Windows CMD Compatibility Fix

## Issue
The original `tauri-dev.bat` used Chinese characters and Emoji, which are not well supported in Windows CMD, even with `chcp 65001` UTF-8 encoding.

## Fix Applied
✅ Replaced all Chinese text with English
✅ Removed all Emoji characters
✅ Used simple ASCII characters only
✅ Maintained all functionality

## Updated Files

### tauri-dev.bat
- ❌ Before: `echo 🚀 Monel Gateway - Tauri 桌面应用`
- ✅ After: `echo Monel Gateway - Tauri Desktop App`

### New File: run.bat
A minimal launcher script:
```batch
@echo off
where rustc >nul 2>nul
if %errorlevel% neq 0 (
    echo ERROR: Rust not installed
    exit /b 1
)
cd src-tauri
cargo tauri dev
```

## Usage

### Option 1: Full version with checks
```cmd
tauri-dev.bat
```

### Option 2: Minimal version
```cmd
run.bat
```

### Option 3: Direct command
```cmd
cd src-tauri
cargo tauri dev
```

## Output Example

Before (broken in CMD):
```
🚀 Monel Gateway - Tauri 桌面应用
✅ 环境检查通过
📦 正在编译 Tauri 应用...
```

After (works in CMD):
```
Monel Gateway - Tauri Desktop App
=====================================
[OK] Environment check passed
Compiling Tauri application...
```

## Compatibility

✅ Windows CMD (cmd.exe)
✅ Windows PowerShell
✅ Git Bash
✅ Windows Terminal
✅ VS Code Terminal

## Notes

- All functionality preserved
- Error messages in English
- Uses standard ASCII characters only
- No encoding issues in any Windows terminal
