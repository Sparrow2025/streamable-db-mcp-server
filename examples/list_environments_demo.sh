#!/bin/bash
# 演示如何使用 list_environments MCP 工具

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🌐 List Environments MCP 工具演示${NC}"
echo "============================================"

# 服务器地址
SERVER_URL="http://localhost:8080/mcp"

echo -e "${YELLOW}📋 1. 列出所有启用的环境${NC}"
echo "请求:"
echo 'curl -X POST '"$SERVER_URL"' -H "Content-Type: application/json" -d '"'"'{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_environments","arguments":{}}}'"'"

echo ""
echo "响应:"
curl -s -X POST "$SERVER_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_environments","arguments":{}}}' | \
  jq '.'

echo ""
echo -e "${YELLOW}📋 2. 列出所有环境（包括禁用的）${NC}"
echo "请求:"
echo 'curl -X POST '"$SERVER_URL"' -H "Content-Type: application/json" -d '"'"'{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_environments","arguments":{"include_disabled":true}}}'"'"

echo ""
echo "响应:"
curl -s -X POST "$SERVER_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_environments","arguments":{"include_disabled":true}}}' | \
  jq '.'

echo ""
echo -e "${YELLOW}📋 3. 提取环境名称列表${NC}"
echo "使用 jq 提取环境名称:"
ENVIRONMENTS=$(curl -s -X POST "$SERVER_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_environments","arguments":{}}}' | \
  jq -r '.result.environments[].name')

echo "可用环境:"
for env in $ENVIRONMENTS; do
    echo "  - $env"
done

echo ""
echo -e "${YELLOW}📋 4. 获取默认环境${NC}"
DEFAULT_ENV=$(curl -s -X POST "$SERVER_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"list_environments","arguments":{}}}' | \
  jq -r '.result.default_environment')

echo "默认环境: $DEFAULT_ENV"

echo ""
echo -e "${YELLOW}📋 5. 检查特定环境的连接信息${NC}"
echo "UAT 环境连接信息:"
curl -s -X POST "$SERVER_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"list_environments","arguments":{}}}' | \
  jq '.result.environments[] | select(.name == "uat") | .connection_info'

echo ""
echo -e "${GREEN}✅ 演示完成！${NC}"
echo ""
echo -e "${YELLOW}💡 使用提示:${NC}"
echo "  - 使用 include_disabled: true 查看所有环境"
echo "  - 响应包含环境状态、连接信息和连接池配置"
echo "  - 可以通过 jq 提取特定信息进行脚本处理"