#!/bin/bash
# 测试 MCP 功能（跳过健康检查）

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🧪 测试 MCP 功能${NC}"
echo "=================================="

# 测试函数
run_test() {
    local test_name="$1"
    local test_command="$2"
    local expected_pattern="$3"
    
    echo -n "  $test_name... "
    
    local result
    result=$(eval "$test_command" 2>&1)
    local exit_code=$?
    
    if [ $exit_code -eq 0 ] && echo "$result" | grep -q "$expected_pattern"; then
        echo -e "${GREEN}✅${NC}"
        return 0
    else
        echo -e "${RED}❌${NC}"
        echo -e "${RED}    错误: $result${NC}"
        return 1
    fi
}

echo -e "${YELLOW}📋 MySQL MCP Server 测试${NC}"

run_test "MCP 初始化" \
    "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}'" \
    '"protocolVersion"'

run_test "工具列表" \
    "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}'" \
    '"execute_query"'

echo ""
echo -e "${YELLOW}🌐 多环境功能测试${NC}"

run_test "列出环境" \
    "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"list_environments\",\"arguments\":{}}}'" \
    '"result"'

run_test "UAT 环境查询" \
    "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"execute_query_env\",\"arguments\":{\"sql\":\"SELECT 1 as test_value\",\"environment\":\"uat\"}}}'" \
    '"result"'

run_test "数据库列表" \
    "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"tools/call\",\"params\":{\"name\":\"list_databases_env\",\"arguments\":{\"environment\":\"uat\"}}}'" \
    '"result"'

echo ""
echo -e "${GREEN}✅ MCP 功能测试完成！${NC}"
echo ""
echo -e "${YELLOW}🔗 服务端点:${NC}"
echo "  - MySQL MCP Server: http://localhost:8080/mcp"
echo "  - MCP-Atlassian: http://localhost:8000/mcp"