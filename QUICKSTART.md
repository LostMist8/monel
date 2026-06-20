# 🚀 快速启动指南

## 🎯 三步启动 Monel Gateway

### ✅ 第 1 步：检查环境

确保已安装 Rust：
```bash
rustc --version
```

如果未安装，访问：https://rustup.rs/

### ✅ 第 2 步：启动应用

**方式 1：使用启动脚本（推荐）**
```bash
# Windows
双击 tauri-dev.bat

# Linux/Mac
chmod +x tauri-dev.sh
./tauri-dev.sh
```

**方式 2：手动启动**
```bash
cd src-tauri
cargo tauri dev
```

### ✅ 第 3 步：使用应用

1. **窗口自动打开** - Tauri 桌面应用
2. **跳过登录** - 内部 Token 自动注入
3. **开始管理** - Provider、模型、统计、日志

---

## 📋 首次运行说明

### 编译时间
- **首次编译**：5-10 分钟（下载依赖 + 编译）
- **后续编译**：30-60 秒（增量编译）

### 期待的输出
```
✅ 环境检查通过
📦 正在编译 Tauri 应用...

   Compiling proc-macro2 v1.0.106
   Compiling unicode-ident v1.0.24
   ...
   Compiling monel-gateway v0.1.0
   
Backend server listening on http://127.0.0.1:7890
Generated internal token for Tauri frontend

🎉 应用窗口已打开！
```

---

## 🎨 界面预览

### 登录页（自动跳过）
```
┌─────────────────────────────┐
│   Monel Gateway             │
│   API Gateway Management    │
│                             │
│   ┌───────────────────────┐ │
│   │ Auth Key (自动填充)   │ │
│   └───────────────────────┘ │
│                             │
│   [ Connect ]               │
└─────────────────────────────┘
```

### 仪表盘
```
┌────────────────────────────────────────┐
│ 📊 Dashboard                           │
│ Overview of your API gateway           │
├────────────────────────────────────────┤
│                                        │
│ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐  │
│ │Online│ │  3   │ │  15  │ │ 1234 │  │
│ │Status│ │Provs │ │Models│ │ Reqs │  │
│ └──────┘ └──────┘ └──────┘ └──────┘  │
│                                        │
│ Server Information                     │
│ ┌────────────────────────────────────┐ │
│ │ Address  127.0.0.1                 │ │
│ │ Port     7890                      │ │
│ │ Auth     Protected                 │ │
│ └────────────────────────────────────┘ │
│                                        │
│ [ Reload Config ]                      │
└────────────────────────────────────────┘
```

---

## 🔧 常见问题

### ❓ 编译错误

**问题：** `cargo tauri dev` 失败

**解决：**
1. 更新 Rust：`rustup update`
2. 清理缓存：`cargo clean`
3. 检查系统依赖

### ❓ 端口被占用

**问题：** `Address already in use (os error 10048)`

**解决：**
1. 修改 `config.yaml` 中的端口
2. 或关闭占用 7890 端口的程序

### ❓ 窗口不显示

**问题：** 编译成功但窗口未打开

**解决：**
1. 检查防火墙设置
2. 查看终端日志
3. 尝试访问 `http://127.0.0.1:7890`

### ❓ 内部 Token 无效

**问题：** 前端显示认证失败

**解决：**
1. 重启应用（Token 每次启动生成）
2. 检查浏览器控制台错误
3. 使用 `config.yaml` 中的 auth_key 手动登录

---

## 📚 下一步

### 配置 Provider

1. 点击左侧 "Providers"
2. 点击 "Add Provider"
3. 填写信息：
   - ID: `openai`
   - Name: `OpenAI`
   - Base URL: `https://api.openai.com/v1`
   - API Key: `sk-your-key`
4. 点击 "Save"

### 测试 API

```bash
# 通过网关调用 OpenAI
curl http://127.0.0.1:7890/chat/openai/chat/completions \
  -H "Authorization: Bearer your-global-secret" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-3.5-turbo",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

### 查看统计

1. 点击左侧 "Dashboard"
2. 查看请求统计
3. 点击 "Logs" 查看详细日志

---

## 🎉 完成！

你现在拥有一个功能完整的 API 网关管理桌面应用！

**主要功能：**
- ✅ 统一管理多个 AI Provider
- ✅ 实时监控请求统计
- ✅ 查看请求日志
- ✅ 动态配置管理
- ✅ 桌面原生体验

**技术特性：**
- ✅ Tauri 2.0 桌面应用
- ✅ Rust 高性能后端
- ✅ 现代暗色 UI
- ✅ 双重鉴权机制
- ✅ 零依赖前端

---

## 📖 相关文档

| 文档 | 说明 |
|------|------|
| `TAURI-README.md` | 详细使用指南 |
| `TAURI-COMPLETION.md` | 开发完成报告 |
| `PROJECT-SUMMARY.md` | 完整项目总结 |
| `USAGE.md` | CLI 模式文档 |

---

## 🆘 需要帮助？

1. **查看日志** - 终端输出包含详细错误信息
2. **检查配置** - `config.yaml` 格式是否正确
3. **阅读文档** - 查看上述相关文档
4. **提交 Issue** - 在 GitHub 上报告问题

---

**祝使用愉快！** 🚀

如有问题，欢迎反馈。
