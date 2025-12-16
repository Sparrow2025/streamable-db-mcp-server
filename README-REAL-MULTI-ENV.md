# 真实多环境 MCP 服务配置

这个配置支持同时运行 MySQL MCP Server (多环境模式) 和 MCP-Atlassian (Jira Cloud) 服务。

## 🏗️ 架构概览

```
┌─────────────────────────────────────────────────────────────┐
│                    Docker Compose                          │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────┐  ┌─────────────────────────────┐  │
│  │  MySQL MCP Server   │  │    MCP-Atlassian           │  │
│  │  (Multi-Environment)│  │    (Jira Cloud)             │  │
│  │  Port: 8080         │  │    Port: 8000               │  │
│  └─────────────────────┘  └─────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
           │                              │
           ▼                              ▼
    ┌─────────────┐                ┌─────────────┐
    │ AWS RDS     │                │ Jira Cloud  │
    │ Aurora      │                │ Instance    │
    │ (UAT)       │                │             │
    └─────────────┘                └─────────────┘
```

## 📁 文件说明

- `docker-compose.real.yml` - Docker Compose 配置文件
- `config.real-multi-env.toml` - MySQL MCP Server 多环境配置
- `start-real-multi-env.sh` - 启动脚本
- `stop-real-multi-env.sh` - 停止脚本

## 🚀 快速开始

### 1. 启动服务

```bash
./start-real-multi-env.sh
```

### 2. 检查服务状态

```bash
# 查看所有服务状态
docker-compose -f docker-compose.real.yml ps

# 查看日志
docker-compose -f docker-compose.real.yml logs -f

# 查看特定服务日志
docker-compose -f docker-compose.real.yml logs -f mysql-mcp-server
docker-compose -f docker-compose.real.yml logs -f mcp-atlassian
```

### 3. 健康检查

```bash
# MySQL MCP Server 健康检查
curl http://localhost:8080/health

# MCP-Atlassian 健康检查
curl http://localhost:8000/health
```

### 4. 停止服务

```bash
./stop-real-multi-env.sh
```

## 🔧 配置说明

### MySQL MCP Server 配置

配置文件: `config.real-multi-env.toml`

```toml
# 默认环境
default_environment = "uat"

[server]
port = 8080
log_level = "info"

# UAT 环境 (当前 AWS RDS Aurora)
[environments.uat]
name = "uat"
description = "User Acceptance Testing environment - AWS RDS Aurora"
enabled = true

[environments.uat.database]
host = "dcs-uat-rds-aurora-cluster.cluster-czcmoige2cq2.ap-southeast-1.rds.amazonaws.com"
port = 3306
username = "web3-rds"
password = "k9egewNGv"
database = "information_schema"
connection_timeout = 30

[environments.uat.connection_pool]
max_connections = 10
min_connections = 2
connection_timeout = 30
idle_timeout = 600
```

### MCP-Atlassian 配置

环境变量配置 (在 docker-compose.real.yml 中):

```yaml
environment:
  JIRA_URL: https://your-domain.atlassian.net
  JIRA_USERNAME: your-email@example.com
  JIRA_API_TOKEN: your-jira-api-token-here
  TRANSPORT: streamable-http
  PORT: 8000
  HOST: 0.0.0.0
  MCP_VERBOSE: "true"
  MCP_LOGGING_STDOUT: "true"
```

## 🌐 服务端点

| 服务 | 端口 | 端点 | 描述 |
|------|------|------|------|
| MySQL MCP Server | 8080 | `/mcp` | MCP 协议端点 |
| MySQL MCP Server | 8080 | `/health` | 健康检查 |
| MySQL MCP Server | 8080 | `/stream/query` | 流式查询端点 |
| MCP-Atlassian | 8000 | `/` | MCP 协议端点 |
| MCP-Atlassian | 8000 | `/health` | 健康检查 |

## 🛠️ 多环境功能

### 可用的 MCP 工具

MySQL MCP Server 提供以下多环境工具:

- `list_environments` - 列出所有可用环境
- `execute_query_env` - 在指定环境执行查询
- `execute_query_multi_env` - 在多个环境执行查询并比较结果
- `list_databases_env` - 列出指定环境的数据库
- `health_check_env` - 检查指定环境的健康状态
- `compare_schema` - 比较不同环境的架构差异

### 使用示例

```bash
# 列出所有环境
curl -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_environments","arguments":{}}}'

# 列出所有环境（包括禁用的）
curl -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_environments","arguments":{"include_disabled":true}}}'

# 在 UAT 环境执行查询
curl -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"execute_query_env","arguments":{"sql":"SELECT 1","environment":"uat"}}}'

# 检查 UAT 环境健康状态
curl -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"health_check_env","arguments":{"environment":"uat"}}}'
```

## 🔒 安全注意事项

1. **敏感信息**: 配置文件包含数据库密码和 API 令牌，请勿提交到版本控制
2. **网络访问**: 服务绑定到 localhost，仅本地访问
3. **资源限制**: 已配置内存和 CPU 限制
4. **日志管理**: 配置了日志轮转以防止磁盘空间耗尽

## 🐛 故障排除

### 常见问题

1. **端口冲突**
   ```bash
   # 检查端口占用
   lsof -i :8080
   lsof -i :8000
   ```

2. **数据库连接失败**
   ```bash
   # 查看 MySQL MCP Server 日志
   docker-compose -f docker-compose.real.yml logs mysql-mcp-server
   ```

3. **Jira 连接失败**
   ```bash
   # 查看 MCP-Atlassian 日志
   docker-compose -f docker-compose.real.yml logs mcp-atlassian
   ```

### 重启服务

```bash
# 重启所有服务
docker-compose -f docker-compose.real.yml restart

# 重启特定服务
docker-compose -f docker-compose.real.yml restart mysql-mcp-server
docker-compose -f docker-compose.real.yml restart mcp-atlassian
```

## 📊 监控和日志

### 查看实时日志

```bash
# 所有服务日志
docker-compose -f docker-compose.real.yml logs -f

# 特定服务日志
docker-compose -f docker-compose.real.yml logs -f mysql-mcp-server
docker-compose -f docker-compose.real.yml logs -f mcp-atlassian
```

### 资源使用情况

```bash
# 查看容器资源使用
docker stats mysql-mcp-server-real-multi mcp-atlassian-real
```

## 🔄 更新和维护

### 更新镜像

```bash
# 拉取最新镜像
docker-compose -f docker-compose.real.yml pull

# 重新构建并启动
docker-compose -f docker-compose.real.yml up --build -d
```

### 备份配置

```bash
# 备份配置文件
cp config.real-multi-env.toml config.real-multi-env.toml.backup
cp docker-compose.real.yml docker-compose.real.yml.backup
```