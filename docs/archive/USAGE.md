# Monel Gateway 使用文档

一个轻量级、透明的 OpenAI 兼容 API 网关。将多个 LLM 提供商统一到单一端点，完全透传请求和响应（包括 SSE 流式）。

## 快速开始

### 编译

```bash
cargo build --release
```

编译产物位于 `target/release/gateway`（Linux/macOS）或 `target/release/gateway.exe`（Windows）。

### 启动服务端

```bash
# 使用默认配置文件 ./config.yaml
./gateway server

# 指定配置文件路径
./gateway server --config /path/to/config.yaml
```

启动成功后会输出：

```
gateway listening on http://127.0.0.1:7890
config file: ./config.yaml
```

### 配置文件

编辑 `config.yaml`：

```yaml
server:
  host: "127.0.0.1"    # 监听地址
  port: 7890            # 监听端口
  auth_key: "your-secret"  # 全局鉴权密钥，留空则无需鉴权

providers:
  - id: "rc"                          # 唯一标识（用于 URL 路由）
    name: "RC (right.codes AWS)"       # 显示名称
    base_url: "https://right.codes/claude-aws/v1"  # 上游 API 地址
    api_key: "sk-xxxxx"               # 转发时使用的 API Key

  - id: "zhipu"
    name: "智谱GLM"
    base_url: "https://open.bigmodel.cn/api/paas/v4"
    api_key: "zhipu-key"
```

修改配置文件后**无需重启**，网关会自动热重载（约 300ms 防抖）。

## 鉴权

所有需要鉴权的端点支持两种方式传递密钥：

| 方式 | 示例 |
|------|------|
| Query 参数 | `?key=your-secret` |
| Bearer Header | `Authorization: Bearer your-secret` |

如果 `server.auth_key` 留空，则所有端点免鉴权。

## API 端点

### 公开端点（无需鉴权）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/health` | 健康检查，返回 `ok` |

### 代理透传端点

所有 OpenAI 兼容端点均可通过以下路径访问：

```
<METHOD> /chat/{provider_id}/{path}
```

路径中的 `v1` 前缀会被自动去除（因为 `base_url` 通常已包含）。

#### 常用端点示例

```bash
# 获取模型列表
curl "http://127.0.0.1:7890/chat/rc/v1/models?key=your-secret"

# 聊天补全（非流式）
curl -X POST "http://127.0.0.1:7890/chat/rc/v1/chat/completions?key=your-secret" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-opus-4-7",
    "messages": [{"role": "user", "content": "hello"}]
  }'

# 聊天补全（SSE 流式）
curl -X POST "http://127.0.0.1:7890/chat/rc/v1/chat/completions?key=your-secret" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-opus-4-7",
    "messages": [{"role": "user", "content": "hello"}],
    "stream": true
  }'

# 文本嵌入
curl -X POST "http://127.0.0.1:7890/chat/zhipu/v1/embeddings?key=your-secret" \
  -H "Content-Type: application/json" \
  -d '{"model": "embedding-3", "input": "hello"}'

# 使用 Bearer Header 方式鉴权
curl "http://127.0.0.1:7890/chat/rc/v1/models" \
  -H "Authorization: Bearer your-secret"
```

#### 透传行为说明

- **请求**：Header（`Authorization` 被替换为上游 `api_key`）、Body、Query 参数原样转发
- **响应**：状态码、Header、Body（含 SSE 流）原样透传
- **错误**：上游返回的错误码（401、500 等）直接透传，不做包装
- **上游不可达**：返回 502 Bad Gateway

### 聚合端点

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/models` | 聚合所有 provider 的模型列表 |
| GET | `/providers` | 列出所有 provider 配置（不含 `api_key`） |

#### `/models` 响应示例

```json
[
  {
    "provider_id": "rc",
    "model": "claude-opus-4-7",
    "name": "RC (right.codes AWS)"
  },
  {
    "provider_id": "zhipu",
    "model": "glm-4",
    "name": "智谱GLM"
  }
]
```

> 单个 provider 超时或失败时自动跳过，始终返回部分结果。

#### `/providers` 响应示例

```json
[
  {
    "id": "rc",
    "name": "RC (right.codes AWS)",
    "base_url": "https://right.codes/claude-aws/v1"
  }
]
```

### 管理端点

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/admin/config` | 获取完整配置（含 `api_key`） |
| POST | `/admin/config` | 替换配置并持久化到磁盘 |
| POST | `/admin/reload` | 强制从磁盘重新读取配置 |

```bash
# 获取当前配置
curl http://127.0.0.1:7890/admin/config -H "Authorization: Bearer your-secret"

# 更新配置（JSON Body）
curl -X POST http://127.0.0.1:7890/admin/config \
  -H "Authorization: Bearer your-secret" \
  -H "Content-Type: application/json" \
  -d '{"server":{"host":"127.0.0.1","port":7890,"auth_key":"new-secret"},"providers":[]}'

# 强制重新加载配置文件
curl -X POST http://127.0.0.1:7890/admin/reload -H "Authorization: Bearer your-secret"
```

## 路径转换规则

用户请求路径中的 `v1` 段会被自动去除，因为 `base_url` 通常已包含该段：

| 用户请求 | Provider base_url | 实际转发到 |
|----------|-------------------|------------|
| `/chat/rc/v1/models` | `https://right.codes/claude-aws/v1` | `https://right.codes/claude-aws/v1/models` |
| `/chat/rc/v1/chat/completions` | `https://right.codes/claude-aws/v1` | `https://right.codes/claude-aws/v1/chat/completions` |
| `/chat/zhipu/v1/embeddings` | `https://open.bigmodel.cn/api/paas/v4` | `https://open.bigmodel.cn/api/paas/v4/embeddings` |

## 错误响应格式

网关自身产生的错误使用 OpenAI 风格的 JSON 信封：

```json
{
  "error": {
    "message": "unknown provider: nonexistent"
  }
}
```

常见状态码：

| 状态码 | 含义 |
|--------|------|
| 200 | 成功 |
| 401 | 鉴权失败（密钥无效或缺失） |
| 404 | Provider 不存在 |
| 405 | 请求方法不允许 |
| 502 | 上游不可达或连接失败 |

## 日志

使用 `RUST_LOG` 环境变量控制日志级别：

```bash
# 默认：info 级别
./gateway server

# 开启 debug 日志
RUST_LOG=debug ./gateway server

# 仅显示网关日志，隐藏 tower_http
RUST_LOG=gateway=debug,tower_http=warn ./gateway server
```

## 关闭

按 `Ctrl+C` 优雅关闭。服务器会等待正在处理的请求完成后退出。
