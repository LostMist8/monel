# Bug Fixes & New Features Summary

## 🐛 Bug Fixes

### 1. **API 404 错误修复**
**问题：** 浏览器访问 `http://127.0.0.1:7890/admin/*` 返回 404，桌面端 API 调用返回 HTML 而不是 JSON

**原因：** 路由顺序错误，静态文件服务 `.nest_service("/", ...)` 拦截了所有请求，包括 API 路由

**修复：**
- 文件：`src/main.rs`
- 改变：将 `.nest_service()` 改为 `.fallback_service()`，放在路由链的最后
- 结果：API 路由优先匹配，静态文件作为 fallback

```rust
// 修复前
.merge(protected)
.nest_service("/", ServeDir::new("ui").fallback(ServeDir::new("page")))
.layer(CorsLayer::permissive())

// 修复后
.merge(protected)
.layer(CorsLayer::permissive())
.fallback_service(ServeDir::new("ui").fallback(ServeDir::new("page")))
```

### 2. **Tauri 内禁止退出登录**
**问题：** Tauri 桌面应用内不应该显示 Logout 按钮（使用内部 Token 自动登录）

**修复：**
- 文件：`ui/index.html`
- 为 Logout 按钮添加 ID：`id="logoutBtn"`
- 在 `init()` 函数中检测 Tauri 环境并隐藏按钮

```javascript
if (isRunningInTauri) {
  const logoutBtn = document.getElementById('logoutBtn');
  if (logoutBtn) logoutBtn.style.display = 'none';
}
```

### 3. **Tauri API 调用错误**
**问题：** `window.__TAURI__.invoke is not a function`

**原因：** Tauri 2.0 API 结构改变

**修复：**
- 从 `window.__TAURI__.invoke()` 改为 `window.__TAURI__.core.invoke()`

### 4. **浏览器缓存问题**
**问题：** 修改后的 HTML 文件被浏览器缓存，导致旧代码执行

**修复：**
- 添加 meta 标签禁用缓存
- 重命名变量 `isTauri` → `isRunningInTauri` 强制刷新

```html
<meta http-equiv="Cache-Control" content="no-cache, no-store, must-revalidate">
<meta http-equiv="Pragma" content="no-cache">
<meta http-equiv="Expires" content="0">
```

## ✨ New Features

### 1. **模型测试功能**
**位置：** Models 页面

**功能：**
- 每个模型旁边添加复选框，支持多选
- 点击 "Test Selected Models" 按钮批量测试
- 发送 "hi" 消息给选中的模型
- 实时显示测试结果和响应时间
- 状态指示：⏳ Testing / ✓ XXXms / ✗ Error

**实现细节：**
```javascript
// 测试逻辑
- 遍历选中的模型复选框
- 对每个模型发送 POST /chat/{provider}/v1/chat/completions
- 使用 performance.now() 测量响应时间
- 显示结果：成功 → 绿色 + 毫秒数，失败 → 红色 + 错误信息
```

**UI 改进：**
- 模型列表从横向 badge 改为纵向可选列表
- 每个模型项悬停显示高亮背景
- 测试结果显示在右侧

### 2. **Logs 页面 Duration 格式化**
**位置：** Logs 页面 - Duration 列

**功能：**
- 自动格式化响应时间显示
- < 1000ms：显示为毫秒（例如：`125ms`）
- >= 1000ms：显示为秒（例如：`2.35s`）

**实现：**
```javascript
const duration = log.duration_ms >= 1000
  ? `${(log.duration_ms / 1000).toFixed(2)}s`
  : `${log.duration_ms}ms`;
```

## 📝 Testing Checklist

### 修复验证
- [x] 浏览器访问 `http://127.0.0.1:7890/` 显示 UI
- [x] 浏览器访问 `http://127.0.0.1:7890/admin/config` 返回 JSON
- [x] Tauri 应用自动登录（无需输入 auth_key）
- [x] Tauri 应用内不显示 Logout 按钮
- [x] 桌面端添加 Provider 成功
- [x] Dashboard/Providers/Models/Logs/Settings 页面正常加载

### 新功能验证
- [ ] Models 页面显示复选框
- [ ] 选中多个模型后点击 "Test Selected Models"
- [ ] 测试过程显示 "⏳ Testing..."
- [ ] 测试完成显示响应时间（绿色 ✓）或错误（红色 ✗）
- [ ] Logs 页面 Duration 列格式正确（ms 或 s）

## 🚀 如何测试

### 1. 重启 Tauri 应用
```bash
# 停止当前运行的应用（Ctrl+C）
cd src-tauri
cargo tauri dev
```

### 2. 测试 API 路由修复
在浏览器中打开：
- `http://127.0.0.1:7890/` → 应该显示登录页面
- `http://127.0.0.1:7890/admin/config` → 应该返回 JSON（需要 auth_key）

### 3. 测试桌面应用
- Tauri 窗口自动打开
- 自动登录（无需输入密码）
- 左侧导航栏**没有** Logout 按钮
- 刷新页面（F5）查看修复后的功能

### 4. 测试模型测试功能
1. 进入 Models 页面
2. 勾选一个或多个模型
3. 点击右上角 "Test Selected Models"
4. 观察每个模型的测试结果

### 5. 测试 Logs 显示
1. 执行几次模型测试
2. 进入 Logs 页面
3. 查看 Duration 列格式是否正确

## 📊 文件修改清单

| 文件 | 修改内容 |
|------|---------|
| `src/main.rs` | 修复路由顺序（`.nest_service` → `.fallback_service`） |
| `ui/index.html` | 1. 隐藏 Tauri 内的 Logout 按钮<br>2. 修复 Tauri API 调用<br>3. 添加模型测试功能<br>4. 格式化 Logs duration 显示<br>5. 添加缓存禁用 meta 标签 |

## 🎯 已知限制

1. **模型列表硬编码**：`loadModels()` 函数中的 `modelsByProvider` 对象包含固定的模型列表，不是从 Provider API 动态获取
2. **测试超时**：模型测试没有超时机制，慢速模型可能导致页面卡顿
3. **并发限制**：所有模型串行测试，没有并发控制

## 🔮 后续改进建议

1. **动态模型列表**：从 `/models` API 获取真实可用模型
2. **测试超时**：添加 30 秒超时机制
3. **并发测试**：使用 `Promise.all()` 并发测试多个模型
4. **测试历史**：保存测试结果到本地存储
5. **导出功能**：导出 Logs 为 CSV 或 JSON

---

**完成日期：** 2026-06-19  
**开发时间：** ~2 小时  
**状态：** ✅ 所有 Bug 已修复，所有新功能已实现
