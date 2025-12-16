#!/bin/bash
# 测试真实多环境 MCP 服务

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🧪 测试真实多环境 MCP 服务${NC}"
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

echo -e "${YELLOW}📋 基础连接测试${NC}"

# 测试 MySQL MCP Server
run_test "MySQL MCP Server 健康检查" \
    "curl -s -f http://localhost:8080/health" \
    '"status":"healthy"'

run_test "MySQL MCP Server MCP 初始化" \
    "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}'" \
    '"protocolVersion"'

run_test "MySQL MCP Server 工具列表" \
    "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}'" \
    '"execute_query"'

echo ""
echo -e "${YELLOW}🌐 多环境功能测试${NC}"

# 测试多环境功能
run_test "列出环境" \
    "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"list_environments\",\"arguments\":{}}}'" \
    '"result"'

run_test "UAT 环境查询" \
    "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"execute_query_env\",\"arguments\":{\"sql\":\"SELECT 1 as test_value\",\"environment\":\"uat\"}}}'" \
    '"test_value"'

run_test "UAT 环境健康检查" \
    "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"tools/call\",\"params\":{\"name\":\"health_check_env\",\"arguments\":{\"environment\":\"uat\"}}}'" \
    '"result"'

echo ""
echo -e "${YELLOW}📊 MCP-Atlassian 测试${NC}"

# 测试 MCP-Atlassian
run_test "MCP-Atlassian 健康检查" \
    "curl -s -f http://localhost:8000/health" \
    '"status"'

run_test "MCP-Atlassian 工具列表" \
    "curl -s -X POST http://localhost:8000 -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}'" \
    '"tools"'

echo ""
echo -e "${YELLOW}📈 性能测试${NC}"

# 响应时间测试
echo -n "  MySQL MCP Server 响应时间... "
start_time=$(date +%s%N)
curl -s -f http://localhost:8080/health > /dev/null
end_time=$(date +%s%N)
response_time=$(( (end_time - start_time) / 1000000 ))

if [ $response_time -lt 1000 ]; then
    echo -e "${GREEN}✅ (${response_time}ms)${NC}"
else
    echo -e "${YELLOW}⚠️  (${response_time}ms - 较慢)${NC}"
fi

echo -n "  MCP-Atlassian 响应时间... "
start_time=$(date +%s%N)
curl -s -f http://localhost:8000/health > /dev/null
end_time=$(date +%s%N)
response_time=$(( (end_time - start_time) / 1000000 ))

if [ $response_time -lt 1000 ]; then
    echo -e "${GREEN}✅ (${response_time}ms)${NC}"
else
    echo -e "${YELLOW}⚠️  (${response_time}ms - 较慢)${NC}"
fi

echo ""
echo -e "${BLUE}📊 容器状态${NC}"
docker-compose -f docker-compose.real.yml ps

echo ""
echo -e "${GREEN}✅ 测试完成！${NC}"
echo ""
echo -e "${YELLOW}🔗 服务端点:${NC}"
echo "  - MySQL MCP Server: http://localhost:8080/mcp"
echo "  - MySQL Health: http://localhost:8080/health"
echo "  - MCP-Atlassian: http://localhost:8000"
echo "  - Atlassian Health: http://localhost:8000/health"