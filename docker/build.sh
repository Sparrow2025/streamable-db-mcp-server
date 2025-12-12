#!/bin/bash

# Docker build script for MySQL MCP Server

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🐳 Building MySQL MCP Server Docker Images${NC}"
echo

# Build production image
echo -e "${YELLOW}Building production image...${NC}"
docker build -t mysql-mcp-server:latest .

# Build development image
echo -e "${YELLOW}Building development image...${NC}"
docker build -f Dockerfile.dev -t mysql-mcp-server:dev .

echo
echo -e "${GREEN}✅ Docker images built successfully!${NC}"
echo
echo "Available images:"
docker images | grep mysql-mcp-server

echo
echo -e "${YELLOW}Next steps:${NC}"
echo "1. Start with docker-compose: docker-compose up -d"
echo "2. Or start development: docker-compose -f docker-compose.dev.yml up -d"
echo "3. Check logs: docker-compose logs -f mysql-mcp-server"
echo "4. Test health: curl http://localhost:8080/health"