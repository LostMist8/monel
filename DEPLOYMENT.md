# 🚀 前后端集成部署指南

## 项目结构

```
monel/
├── src/               # Rust 后端代码
├── page/              # 前端静态文件
│   ├── index.html     # 欢迎页
│   └── admin.html     # 管理后台
├── config.yaml        # 配置文件
└── Cargo.toml         # 依赖配置
```

## 前后端集成说明

### 1. 后端改动

已在 Rust 代码中添加静态文件服务支持：

- **Cargo.toml**：添加了 `tower-http` 的 `fs` 特性
- **main.rs**：
  - 导入 `tower_http::services::ServeDir`
  - 在路由中添加 `.nest_service("/", ServeDir::new("page"))`
  - 静态文件从 `page` 目录提供服务

### 2. 前端页面

**欢迎页** (`page/index.html`)
- URL: `http://127.0.0.1:7890/`
- 显示项目信息和快速链接
- 提供管理后台入口

**管理后台** (`page/admin.html`)
- URL: `http://127.0.0.1:7890/admin.html`
- Material3 设计风格
- 功能：
  - 登录页（Auth Key 认证）
  - 仪表盘（服务状态）
  - Provider 管理（增删改查）
  - 模型列表（按 Provider 分组）
  - 设置（修改服务器配置）

### 3. API 端点

前端通过以下 API 与后端通信：

```
GET  /admin/config   # 获取配置（需认证）
POST /admin/config   # 保存配置（需认证）
POST /admin/reload   # 重载配置（需认证）
GET  /health         # 健康检查（公开）
```

所有认证请求使用 `Authorization: Bearer {auth_key}` header。

## 部署步骤

### 开发环境

1. **启动服务器**
```bash
cargo run -- server
```

2. **访问应用**
```
欢迎页：http://127.0.0.1:7890/
管理后台：http://127.0.0.1:7890/admin.html
```

3. **登录管理后台**
使用 `config.yaml` 中的 `auth_key` 登录

### 生产环境

1. **编译发布版本**
```bash
cargo build --release
```

2. **部署文件**
```
部署目录/
├── gateway            # 可执行文件
├── config.yaml        # 配置文件
└── page/              # 静态文件目录
    ├── index.html
    └── admin.html
```

3. **启动服务**
```bash
./gateway server --config config.yaml
```

4. **使用反向代理（推荐）**

Nginx 配置示例：
```nginx
server {
    listen 80;
    server_name api.example.com;

    location / {
        proxy_pass http://127.0.0.1:7890;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_cache_bypass $http_upgrade;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
}
```

## 使用示例

### 1. 访问欢迎页
打开浏览器访问 `http://127.0.0.1:7890/`

### 2. 登录管理后台
1. 点击"管理后台"按钮
2. 输入 `config.yaml` 中配置的 `auth_key`
3. 点击"连接"

### 3. 管理 Provider
- 查看现有 Provider
- 添加新 Provider（填写 ID、名称、Base URL、API Key）
- 编辑 Provider 配置
- 删除 Provider（带确认对话框）
- 切换 API Key 显示/隐藏

### 4. 查看模型
- 按 Provider 分组展示
- 点击展开/折叠每个 Provider 的模型列表

### 5. 修改设置
- 修改监听地址和端口
- 修改 Auth Key
- 保存后配置会立即生效

## 安全建议

1. **修改默认 Auth Key**
```yaml
server:
  auth_key: "使用强随机密码"
```

2. **限制监听地址**（生产环境）
```yaml
server:
  host: "127.0.0.1"  # 仅本地访问，通过反向代理暴露
```

3. **启用 HTTPS**（使用 Nginx 等反向代理）

4. **保护配置文件**
```bash
chmod 600 config.yaml
```

## 故障排除

### 页面无法加载
- 确认 `page` 目录存在且包含 HTML 文件
- 检查文件路径是否正确
- 查看服务器日志

### 管理后台无法连接
- 确认输入了正确的 Auth Key
- 检查浏览器控制台是否有错误
- 确认后端 API 正常响应（访问 `/health`）

### 配置保存失败
- 检查 YAML 格式是否正确
- 确认有写入 `config.yaml` 的权限
- 查看服务器日志获取详细错误

## 技术栈

**后端**
- Rust
- Axum (Web 框架)
- Tower-HTTP (中间件)
- Tokio (异步运行时)

**前端**
- 原生 HTML/CSS/JavaScript
- Material3 设计语言
- Material Icons
- Roboto 字体

## 更新日志

- ✅ 添加静态文件服务支持
- ✅ 创建 Material3 风格管理界面
- ✅ 实现完整的 CRUD 功能
- ✅ 添加欢迎页
- ✅ 集成前后端

## 后续改进

- [ ] 添加用户管理
- [ ] 支持多语言
- [ ] 添加日志查看功能
- [ ] 实时监控和统计
- [ ] 深色模式支持
