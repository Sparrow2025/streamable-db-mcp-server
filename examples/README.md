# MySQL MCP Server - 使用示例

这个目录包含了演示如何使用 MySQL MCP Server 多环境功能的示例代码。

## 📁 文件说明

### 🌐 List Environments 工具示例

- **`list_environments_demo.sh`** - Bash 脚本示例
- **`list_environments_demo.py`** - Python 脚本示例  
- **`list_environments_demo.js`** - Node.js 脚本示例

### 🔧 其他示例

- **`enhanced_mcp_tools_demo.rs`** - Rust 代码中的 MCP 工具使用示例
- **`environment_manager_demo.rs`** - 环境管理器使用示例

## 🚀 运行示例

### 前提条件

确保 MySQL MCP Server 正在运行：

```bash
# 启动多环境服务
./start-real-multi-env.sh

# 或者启动开发环境
docker-compose -f docker/docker-compose.multi-env.yml up -d
```

### Bash 示例

```bash
# 运行 Bash 示例
./examples/list_environments_demo.sh
```

### Python 示例

```bash
# 安装依赖
pip install requests

# 运行 Python 示例
python3 examples/list_environments_demo.py
```

### Node.js 示例

```bash
# 运行 Node.js 示例（无需额外依赖）
node examples/list_environments_demo.js
```

## 📋 List Environments 工具详解

### 基本用法

```bash
# 列出所有启用的环境
curl -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_environments","arguments":{}}}'
```

### 参数说明

| 参数 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `include_disabled` | boolean | 否 | false | 是否包含禁用的环境 |

### 响应格式

```json
{
  "environments": [
    {
      "name": "uat",
      "description": "User Acceptance Testing environment",
      "status": "enabled",
      "is_default": true,
      "is_legacy": false,
      "connection_info": {
        "host": "localhost",
        "port": 3306,
        "database": "test_db",
        "username": "user",
        "password_configured": true
      },
      "pool_config": {
        "max_connections": 10,
        "min_connections": 2,
        "connection_timeout": 30,
        "idle_timeout": 600
      }
    }
  ],
  "total_count": 1,
  "default_environment": "uat"
}
```

### 响应字段说明

#### 环境信息 (environments[])

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 环境名称 |
| `description` | string | 环境描述 |
| `status` | string | 环境状态：enabled/disabled/invalid |
| `is_default` | boolean | 是否为默认环境 |
| `is_legacy` | boolean | 是否为遗留单数据库模式 |

#### 连接信息 (connection_info)

| 字段 | 类型 | 说明 |
|------|------|------|
| `host` | string | 数据库主机地址 |
| `port` | number | 数据库端口 |
| `database` | string | 数据库名称 |
| `username` | string | 用户名 |
| `password_configured` | boolean | 是否配置了密码 |

#### 连接池配置 (pool_config)

| 字段 | 类型 | 说明 |
|------|------|------|
| `max_connections` | number | 最大连接数 |
| `min_connections` | number | 最小连接数 |
| `connection_timeout` | number | 连接超时时间（秒） |
| `idle_timeout` | number | 空闲超时时间（秒） |

#### 根级别字段

| 字段 | 类型 | 说明 |
|------|------|------|
| `total_count` | number | 返回的环境总数 |
| `default_environment` | string | 默认环境名称 |

## 🔍 使用场景

### 1. 环境发现

在连接到 MCP 服务器后，首先调用 `list_environments` 来发现可用的环境：

```python
# Python 示例
client = MySQLMCPClient()
environments = client.list_environments()
print(f"可用环境: {[env['name'] for env in environments['environments']]}")
```

### 2. 环境状态检查

检查哪些环境是启用的，哪些是禁用的：

```javascript
// JavaScript 示例
const allEnvs = await client.listEnvironments(true);
const enabledEnvs = allEnvs.environments.filter(env => env.status === 'enabled');
const disabledEnvs = allEnvs.environments.filter(env => env.status === 'disabled');
```

### 3. 连接信息获取

获取特定环境的连接信息用于监控或调试：

```bash
# Bash 示例
curl -s -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_environments","arguments":{}}}' | \
  jq '.result.environments[] | select(.name == "uat") | .connection_info'
```

### 4. 动态环境选择

根据可用环境动态选择要操作的环境：

```python
# Python 示例
def select_environment(client, preferred_env=None):
    envs = client.list_environments()
    
    if preferred_env:
        for env in envs['environments']:
            if env['name'] == preferred_env and env['status'] == 'enabled':
                return preferred_env
    
    # 回退到默认环境
    return envs['default_environment']
```

## 🛠️ 故障排除

### 常见错误

1. **连接被拒绝**
   ```
   Error: connect ECONNREFUSED 127.0.0.1:8080
   ```
   解决方案：确保 MCP 服务器正在运行

2. **工具不存在**
   ```
   MCP Error: {"code": -32601, "message": "Method not found"}
   ```
   解决方案：确保使用的是多环境版本的 MCP 服务器

3. **无环境返回**
   ```
   {"environments": [], "total_count": 0}
   ```
   解决方案：检查配置文件中是否有启用的环境

### 调试技巧

1. **启用详细日志**
   ```bash
   RUST_LOG=debug ./target/release/mysql-mcp-server
   ```

2. **检查配置文件**
   ```bash
   cat config.toml | grep -A 10 "\[environments\."
   ```

3. **测试连接**
   ```bash
   curl -f http://localhost:8080/health
   ```

## 📚 相关文档

- [MCP 工具参考](../docs/MCP_TOOLS_REFERENCE.md)
- [多环境配置指南](../README-REAL-MULTI-ENV.md)
- [故障排除指南](../docs/TROUBLESHOOTING.md)