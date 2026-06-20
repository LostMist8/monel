# Monel Gateway 二次开发文档

## 项目架构

### 总览

Monel 是一个用 Rust 编写的轻量级 API 网关，基于 **axum + tokio** 异步运行时。核心设计理念是「透明代理」——尽可能少的中间层逻辑，对上游 API 的请求和响应完全透传。

```
┌─────────┐       ┌──────────────────────────────────────┐       ┌─────────────┐
│  Client  │──────▶│           Monel Gateway              │──────▶│  Upstream    │
│ (curl /  │       │                                      │       │  Provider    │
│  SDK)    │◀──────│  Auth → Proxy → Pass-through (SSE)   │◀──────│  (OpenAI     │
│          │       │                                      │       │   兼容 API)  │
└─────────┘       └──────────────────────────────────────┘       └─────────────┘
                            │
                     ┌──────┴──────┐
                     │ config.yaml │ (热重载)
                     │  + watcher  │
                     └─────────────┘
```

### 二进制模式

单二进制通过子命令分发：

| 子命令 | 行为 | 状态 |
|--------|------|------|
| `gateway server [--config <path>]` | 启动常驻后台代理服务 | ✅ 已实现 |
| `gateway ui` | 启动 Slint GUI（通过 HTTP 调用 server 管理） | ⏳ 架构预留 |

### 源码结构

```
src/
├── main.rs          # 入口：CLI 解析、路由组装、文件监听、优雅关闭
├── config.rs        # 配置类型定义、YAML 序列化/反序列化、热重载共享状态
├── state.rs         # 应用全局共享状态（Config + HTTP Client + Config Path）
├── error.rs         # 统一错误类型 → axum IntoResponse
├── auth.rs          # 鉴权中间件（query key / Bearer header / constant-time compare）
├── proxy.rs         # 核心：透明代理透传（路径转换、header 清洗、SSE 流式转发）
├── aggregator.rs    # 聚合端点：/models（并发扇出）和 /providers
└── admin.rs         # 管理端点：GET/POST /admin/config、POST /admin/reload
```

### 模块依赖关系

```
main.rs
  ├── config.rs  ←── SharedConfig (Arc<RwLock<Config>>)
  ├── state.rs   ←── AppState { config, http, config_path }
  ├── auth.rs          使用 config::read() 读取 auth_key
  ├── proxy.rs         使用 state.http 转发请求
  ├── aggregator.rs    使用 state.http 扇出 /models
  └── admin.rs         使用 config::read/replace 操作配置

所有 handler 通过 axum::extract::State<AppState> 获取共享状态。
所有 handler 返回 AppResult<T>（即 Result<T, AppError>）。
```

### 核心数据流（代理请求）

```
1. 请求到达 axum Router
2. auth::require_auth 中间件验证 auth_key（如已配置）
3. proxy::proxy 提取 {provider_id} 和 *path
4. 在 RwLockReadGuard 内查找 provider，获取 base_url + api_key，然后释放锁
5. build_upstream_url: 去除路径中的 v1 前缀，拼接到 base_url
6. sanitize: 清洗请求 header（移除 hop-by-hop），替换 Authorization
7. reqwest 转发请求（body 以 stream 方式透传）
8. 接收上游响应：status + headers（清洗后）+ body stream → 直接返回客户端
```

### 配置热重载机制

```
文件系统事件 (notify crate)
    ↓ (Create/Modify/Remove)
mpsc channel (缓冲 32 条)
    ↓
tokio 异步任务消费
    ↓
300ms 防抖（合并编辑器连续保存事件）
    ↓
Config::load() → 结构对比 → 有变化则 config::replace()
    ↓
Arc<RwLock<Config>> 就地替换，下次请求自动读到新配置
    ↓
当前正在处理的请求使用旧配置，不受影响（无锁竞争风险）
```

## 技术栈

| 依赖 | 版本 | 用途 |
|------|------|------|
| axum | 0.7 | HTTP 框架 |
| tokio | 1 | 异步运行时 |
| reqwest | 0.12 | HTTP 客户端（rustls-tls, stream, json） |
| serde + serde_yaml | 1 / 0.9 | 配置序列化 |
| notify | 6 | 文件变更监听 |
| tower-http | 0.5 | CORS、请求追踪 |
| tracing | 0.1 | 结构化日志 |
| futures | 0.3 | Stream 组合 |
| anyhow | 1 | 错误处理 |
| url | 2 | URL 解析与拼接 |

## 关键设计决策

### 1. Send 安全的 Handler

`RwLockReadGuard` 不是 `Send`，如果持有 guard 跨越 `.await` 点会导致整个 handler future 变为 `!Send`，从而无法满足 axum 的 `Handler` trait 约束。

**解决方案**：在 block scope 内获取 guard、提取所需数据（clone），然后 drop guard 再执行异步操作。

```rust
// proxy.rs 中的做法
let provider = {
    let cfg = crate::config::read(&state.config);  // 获取 guard
    cfg.find_provider(&provider_id)                  // 提取并 clone
        .ok_or_else(|| AppError::not_found(...))?    // guard 在此处 drop
};  // ← guard 已释放，后续 .await 安全
```

添加新 handler 时务必遵循此模式。

### 2. 路径转换中的 v1 去重

OpenAI 兼容 API 的 base_url 通常已包含 `/v1`，而客户端请求路径也以 `/v1/` 开头。为避免重复，proxy 自动去除用户路径中的第一个 `v1` 段。

### 3. Header 清洗

代理会移除 hop-by-hop header（`connection`、`transfer-encoding` 等），防止不兼容的上游 header 影响客户端。请求侧额外移除 `host`、`content-length`、原始 `authorization`（替换为上游 key）。

### 4. 配置原子写入

`Config::save()` 使用「写临时文件 + rename」策略，确保不会出现写入一半的损坏配置文件。

## 待完成模块

### 1. Slint GUI（高优先级）

**目标**：提供可视化的配置管理界面。

**架构**：独立进程，通过 HTTP 调用已有的 `/admin/*` 端点。无需直接读取配置文件或共享内存。

**所需工作**：

- [ ] 添加 `slint` 和 `slint-build` 依赖
- [ ] 创建 `src/ui/` 模块（或独立 binary）
- [ ] 实现 Slint 组件：
  - [ ] Provider 列表展示
  - [ ] 添加 / 编辑 / 删除 Provider
  - [ ] 全局设置（host、port、auth_key）
  - [ ] 实时模型列表查看（调用 GET /models）
- [ ] 实现与 server 端的 HTTP 通信层
- [ ] `gateway ui` 子命令启动 GUI（自动检测 server 是否运行）

**已有的 Admin API**（GUI 可直接使用）：

| 端点 | 说明 |
|------|------|
| `GET /admin/config` | 获取完整配置 |
| `POST /admin/config` | 替换配置并持久化 |
| `POST /admin/reload` | 强制重载磁盘配置 |
| `GET /providers` | Provider 列表（脱敏） |
| `GET /models` | 模型列表聚合 |
| `GET /health` | Server 存活检测 |

### 2. 按路由/Provider 鉴权（中优先级）

**当前行为**：单一全局 `auth_key`，所有 provider 共享。

**建议扩展**：

- 在 `Provider` 配置中添加可选的 `auth_key` 字段
- 鉴权逻辑改为：优先检查 provider 级密钥，回退到全局密钥
- 允许不同 client 访问不同 provider 子集

### 3. 请求/响应日志（中优先级）

**当前行为**：仅有 tracing 结构化日志（请求级别由 tower-http TraceLayer 提供）。

**建议扩展**：

- [ ] 可选的请求/响应 body 日志（开发调试用）
- [ ] 按 provider 分组的请求计数器（内存中，通过 `/admin/stats` 暴露）
- [ ] 可选的请求日志持久化（JSON 文件或 SQLite）

### 4. 速率限制（低优先级）

**建议方案**：

- 基于 `tower::Service` 的中间件
- 按 auth_key 维度的令牌桶/滑动窗口
- 可选配置：每个 provider 独立的并发上限
- 可选配置：全局最大并发数

### 5. Upstream 健康检查与故障转移（低优先级）

**当前行为**：上游不可达直接返回 502。

**建议扩展**：

- [ ] 定期 health check 每个 provider 的 `/models` 端点
- [ ] 标记不健康的 provider，`/models` 聚合时自动跳过
- [ ] 可选：同 ID 多 base_url 的负载均衡（round-robin 或 failover）
- [ ] 可选：超时自动重试（切换到备用地址）

### 6. 请求改写/模型映射（低优先级）

**当前行为**：完全透传请求 body。

**建议扩展**：

- [ ] Provider 级的模型名映射表（客户端发 `gpt-4`，实际转发 `glm-4`）
- [ ] Provider 级的 base prompt / system message 注入
- [ ] 请求 body 字段的过滤或增强

## 开发指南

### 构建与运行

```bash
# 开发构建（快速编译，含调试信息）
cargo build

# 发布构建（优化体积和性能）
cargo build --release

# 运行测试
cargo test

# 格式检查
cargo fmt --check

# Lint 检查
cargo clippy
```

### 添加新端点

1. 在对应模块文件中实现 handler 函数，签名为：

```rust
pub async fn my_handler(
    State(state): State<AppState>,
    // ... 其他 extractor
) -> AppResult<impl IntoResponse> {
    // 如果需要读取配置，在 block 内获取并 clone，不要跨 await 持有 guard
    let something = {
        let cfg = crate::config::read(&state.config);
        cfg.some_field.clone()
    };

    // 异步操作...
    Ok(Json(serde_json::json!({"status": "ok"})))
}
```

2. 在 `main.rs` 的 `build_router()` 中注册路由：

```rust
let protected = Router::<AppState>::new()
    // 已有路由...
    .route("/my-endpoint", axum::routing::get(my_module::my_handler))
    .layer(middleware::from_fn_with_state(state.clone(), auth::require_auth));
```

### 添加新配置字段

1. 在 `config.rs` 的对应 struct 中添加字段，使用 `#[serde(default)]` 确保向后兼容
2. 更新 `config.yaml` 示例
3. 更新 `Config::validate()` 如需校验
4. 更新 `main.rs` 的 `config_equal()` 如需支持热重载变更检测

### 配置文件 schema 参考

```yaml
server:
  host: string       # 监听地址，默认 "127.0.0.1"
  port: u16          # 监听端口，默认 7890
  auth_key: string   # 全局鉴权密钥，默认空（免鉴权）

providers:
  - id: string       # 唯一标识（必填，不可重复）
    name: string     # 显示名称
    base_url: string  # 上游 API 地址（必填）
    api_key: string   # 上游 API Key（必填）
```
