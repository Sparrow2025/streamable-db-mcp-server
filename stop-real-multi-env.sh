#!/bin/bash
# 停止真实多环境 MCP 服务

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${YELLOW}🛑 停止真实多环境 MCP 服务${NC}"
echo "=================================="

# 停止服务
echo -e "${YELLOW}⏹️  停止服务...${NC}"
docker-compose -f docker-compose.real.yml down

echo -e "${GREEN}✅ 服务已停止${NC}"

# 可选：清理未使用的镜像和容器
read -p "是否清理未使用的 Docker 资源? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${YELLOW}🧹 清理 Docker 资源...${NC}"
    docker system prune -f
    echo -e "${GREEN}✅ 清理完成${NC}"
fi