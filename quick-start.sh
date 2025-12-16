#!/bin/bash
# 快速启动脚本 - 仅启动必要服务

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}🚀 快速启动 MySQL MCP Server${NC}"
echo "=================================="

# 停止现有服务
echo -e "${YELLOW}🛑 停止现有服务...${NC}"
docker-compose -f docker-compose.real.yml down 2>/dev/null || true

# 使用 UAT-only 配置
echo -e "${YELLOW}📝 使用 UAT-only 配置...${NC}"
cp config.real-uat-only.toml config.real-multi-env.toml

# 启动服务
echo -e "${YELLOW}🔨 启动服务...${NC}"
docker-compose -f docker-compose.real.yml up -d

# 等待启动
echo -e "${YELLOW}⏳ 等待服务启动 (30秒超时)...${NC}"
timeout=30
while [ $timeout -gt 0 ]; do
    if curl -s -f http://localhost:8080/health > /dev/null 2>&1; then
        echo -e "${GREEN}✅ MySQL MCP Server 启动成功！${NC}"
        break
    fi
    
    if curl -s -f http://localhost:8000/health > /dev/null 2>&1; then
        echo -e "${GREEN}✅ MCP-Atlassian 启动成功！${NC}"
    fi
    
    echo -n "."
    sleep 1
    timeout=$((timeout - 1))
done

if [ $timeout -eq 0 ]; then
    echo -e "\n${RED}❌ 服务启动超时${NC}"
    echo -e "${YELLOW}查看日志:${NC}"
    docker-compose -f docker-compose.real.yml logs --tail=20
    exit 1
fi

echo ""
echo -e "${GREEN}🎉 服务启动完成！${NC}"
echo ""
echo -e "${YELLOW}🔗 服务端点:${NC}"
echo "  - MySQL MCP Server: http://localhost:8080/mcp"
echo "  - MCP-Atlassian: http://localhost:8000"
echo ""
echo -e "${YELLOW}📋 测试命令:${NC}"
echo "  curl http://localhost:8080/health"
echo "  curl http://localhost:8000/health"