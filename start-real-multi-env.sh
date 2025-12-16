#!/bin/bash
# 启动真实多环境 MySQL MCP Server 和 MCP-Atlassian 服务

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}🚀 启动真实多环境 MCP 服务${NC}"
echo "=================================="

# 检查配置文件是否存在
if [ ! -f "config.real-multi-env.toml" ]; then
    echo -e "${RED}❌ 配置文件 config.real-multi-env.toml 不存在${NC}"
    echo "请确保配置文件存在并包含正确的数据库连接信息"
    exit 1
fi

# 检查 Docker 是否运行
if ! docker info > /dev/null 2>&1; then
    echo -e "${RED}❌ Docker 未运行，请先启动 Docker${NC}"
    exit 1
fi

echo -e "${YELLOW}📋 服务配置:${NC}"
echo "  - MySQL MCP Server (多环境): http://localhost:8080"
echo "  - MCP-Atlassian (Jira): http://localhost:8000"
echo ""

# 构建并启动服务
echo -e "${YELLOW}🔨 构建并启动服务...${NC}"
docker-compose -f docker-compose.real.yml up --build -d

# 等待服务启动
echo -e "${YELLOW}⏳ 等待服务启动...${NC}"
sleep 10

# 检查服务状态
echo -e "${YELLOW}🔍 检查服务状态...${NC}"
docker-compose -f docker-compose.real.yml ps

# 健康检查
echo ""
echo -e "${YELLOW}🏥 执行健康检查...${NC}"

# 检查 MySQL MCP Server
echo -n "  MySQL MCP Server... "
if curl -s -f http://localhost:8080/health > /dev/null 2>&1; then
    echo -e "${GREEN}✅ 健康${NC}"
else
    echo -e "${RED}❌ 不健康${NC}"
fi

# 检查 MCP-Atlassian
echo -n "  MCP-Atlassian... "
if curl -s -f http://localhost:8000/health > /dev/null 2>&1; then
    echo -e "${GREEN}✅ 健康${NC}"
else
    echo -e "${RED}❌ 不健康${NC}"
fi

echo ""
echo -e "${GREEN}✅ 服务启动完成！${NC}"
echo ""
echo -e "${YELLOW}📖 使用说明:${NC}"
echo "  - 查看日志: docker-compose -f docker-compose.real.yml logs -f"
echo "  - 停止服务: docker-compose -f docker-compose.real.yml down"
echo "  - 重启服务: docker-compose -f docker-compose.real.yml restart"
echo ""
echo -e "${YELLOW}🔗 服务端点:${NC}"
echo "  - MySQL MCP Server: http://localhost:8080/mcp"
echo "  - MySQL Health Check: http://localhost:8080/health"
echo "  - MCP-Atlassian: http://localhost:8000"
echo "  - Atlassian Health Check: http://localhost:8000/health"