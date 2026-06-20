# Monel Gateway - Tauri Desktop App

现代化的 API 网关管理桌面应用，使用 Tauri 2.0 + Rust + Axum 构建。

## ✨ 改进亮点

### 设计升级
- ✅ **现代暗色主题** - 从 Material Purple 改为精致的深色调色板
- ✅ **Inter + JetBrains Mono** - 替代通用 Roboto，数据使用等宽字体
- ✅ **增强的视觉层次** - 更强的字重对比（700 vs 400）
- ✅ **优化的间距系统** - 基于 4px 网格的一致间距
- ✅ **改进的交互反馈** - :active 状态，流畅过渡（250ms）
- ✅ **单一强调色** - 蓝色 (#3b82f6) 取代 AI 紫色
- ✅ **深度阴影** - 带颜色的阴影而非纯黑
- ✅ **视口稳定性** - 使用 `100dvh` 而非 `100vh`

### 技术改进
- ✅ Tauri 2.0 桌面应用
- ✅ 内部 Token 认证（前端免鉴权）
- ✅ 统计和日志功能
- ✅ 响应式设计
- ✅ 暗色模式优化
- ✅ 自定义滚动条样式

## 🚀 快速开始

### 前置要求

1. **Rust** (最新稳定版)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Node.js & npm** (可选，用于 Tauri CLI)
   ```bash
   npm install -g @tauri-apps/cli
   ```

3. **系统依赖**
   - Windows: 已包含所有依赖
   - macOS: `xcode-select --install`
   - Linux: `sudo apt install libwebkit2gtk-4.0-dev build-essential curl wget`

### 开发模式

```bash
# 1. 克隆项目
cd monel

# 2. 安装依赖并运行
cd src-tauri
cargo tauri dev
```

### 生产构建

```bash
cd src-tauri
cargo tauri build
```

构建产物在 `src-tauri/target/release/bundle/`

## 📁 项目结构

```
monel/
├── src/                    # 原始 Rust 后端代码
│   ├── main.rs            # CLI 模式入口
│   ├── config.rs          # 配置管理
│   ├── auth.rs            # 认证（支持内部 token）
│   ├── stats.rs           # 统计模块（新增）
│   └── ...
├── src-tauri/             # Tauri 应用
│   ├── Cargo.toml         # Tauri 依赖
│   ├── tauri.conf.json    # Tauri 配置
│   └── src/
│       └── main.rs        # Tauri 入口（启动后端 + 窗口）
├── ui/                    # 前端界面
│   └── index.html         # 单页应用（升级版设计）
└── config.yaml            # 网关配置文件
```

## 🎨 设计系统

### 颜色
```css
/* 背景 */
--color-bg-primary: #0a0a0a      /* 主背景 - 深灰黑 */
--color-bg-secondary: #111111    /* 次级背景 */
--color-bg-elevated: #1a1a1a     /* 悬浮元素 */

/* 文本 */
--color-text-primary: #e5e5e5    /* 主文本 */
--color-text-secondary: #a3a3a3  /* 次级文本 */

/* 强调色 */
--color-accent: #3b82f6          /* 蓝色（单一强调色）*/
```

### 字体
- **Sans-serif**: Inter (400, 500, 600, 700)
- **Monospace**: JetBrains Mono (用于代码、数字)

### 圆角
- Small: 6px
- Medium: 8px
- Large: 12px

## 🔧 功能说明

### 1. 仪表盘
- 服务状态监控
- Provider 和模型统计
- 请求总数
- 服务器信息
- 配置重载按钮

### 2. Provider 管理
- 列表展示所有 Provider
- 添加/编辑/删除 Provider
- API Key 显示/隐藏切换
- 实时保存到 `config.yaml`

### 3. 模型列表
- 按 Provider 分组展示
- 显示每个 Provider 的可用模型

### 4. 请求日志
- 显示最近的 API 请求
- 时间、Provider、模型、状态码、耗时

### 5. 设置
- 修改监听地址和端口
- 修改 Auth Key
- 实时保存配置

## 🔐 鉴权说明

### 双重鉴权机制

1. **Tauri 窗口内的请求** → 使用内部 Token（自动生成）
   - 启动时生成随机 Token
   - 前端通过 `__TAURI__.invoke('get_internal_token')` 获取
   - 后端识别此 Token 直接放行

2. **外部 HTTP 请求** → 使用 `auth_key`（用户配置）
   - curl / 其他程序访问 `http://127.0.0.1:7890`
   - 需要提供 `Authorization: Bearer {auth_key}`
   - 在 `config.yaml` 中配置

### 示例

```bash
# Tauri 窗口内 - 无需手动提供 Token
# 前端自动使用内部 Token

# 外部 API 调用 - 需要提供 auth_key
curl http://127.0.0.1:7890/admin/config \
  -H "Authorization: Bearer your-global-secret"
```

## 📊 统计功能（新增）

### API 端点

- `GET /admin/stats` - 获取统计数据
  ```json
  {
    "total_requests": 1234,
    "total_errors": 5,
    "providers": {
      "openai": {
        "request_count": 800,
        "avg_duration_ms": 250,
        "error_count": 2
      }
    },
    "models": {
      "gpt-4": {
        "request_count": 500,
        "avg_duration_ms": 300
      }
    }
  }
  ```

- `GET /admin/logs?limit=100` - 获取请求日志
  ```json
  [
    {
      "timestamp": 1234567890,
      "provider": "openai",
      "model": "gpt-4",
      "status_code": 200,
      "duration_ms": 250,
      "error": null
    }
  ]
  ```

## 🛠️ 开发说明

### 修改前端

编辑 `ui/index.html`，然后刷新 Tauri 窗口（Ctrl+R）

### 修改后端

1. 编辑 `src/` 下的 Rust 代码
2. 重启 `cargo tauri dev`

### 调试

```bash
# 查看后端日志
RUST_LOG=debug cargo tauri dev

# 查看前端控制台
右键 → Inspect Element → Console
```

## 🎯 后续改进建议

- [ ] 完整的后端集成（目前简化版）
- [ ] 实时 WebSocket 更新
- [ ] 图表可视化统计
- [ ] 深色/浅色模式切换
- [ ] 多语言支持
- [ ] 导出/导入配置
- [ ] Provider 健康检查
- [ ] 请求限流配置

## 📝 配置文件

`config.yaml`:
```yaml
server:
  host: "127.0.0.1"
  port: 7890
  auth_key: "your-global-secret"

providers:
  - id: "openai"
    name: "OpenAI"
    base_url: "https://api.openai.com/v1"
    api_key: "sk-..."

  - id: "anthropic"
    name: "Anthropic"
    base_url: "https://api.anthropic.com/v1"
    api_key: "sk-ant-..."
```

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 许可证

[MIT License](LICENSE)
