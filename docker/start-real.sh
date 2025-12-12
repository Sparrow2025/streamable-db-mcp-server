#!/bin/bash

# Start script for real AWS RDS Aurora environment
# WARNING: This connects to production/UAT database
# Use with caution!

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}🚨 WARNING: Starting MCP Server with REAL AWS RDS Aurora Database${NC}"
echo -e "${YELLOW}   Database: dcs-uat-rds-aurora-cluster.cluster-czcmoige2cq2.ap-southeast-1.rds.amazonaws.com${NC}"
echo -e "${YELLOW}   This will connect to the actual UAT database!${NC}"
echo ""

# Confirm with user
read -p "Are you sure you want to continue? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${RED}❌ Aborted by user${NC}"
    exit 1
fi

echo -e "${GREEN}🔧 Building and starting MCP server with real database...${NC}"

# Check if docker-compose.real.yml exists
if [ ! -f "docker-compose.real.yml" ]; then
    echo -e "${RED}❌ Error: docker-compose.real.yml not found${NC}"
    echo "Please make sure you have created the real configuration file."
    exit 1
fi

# Build the image first
echo -e "${GREEN}🏗️  Building Docker image...${NC}"
docker-compose -f docker-compose.real.yml build

# Start the services
echo -e "${GREEN}🚀 Starting services...${NC}"
docker-compose -f docker-compose.real.yml up -d

# Wait a moment for services to start
echo -e "${GREEN}⏳ Waiting for services to start...${NC}"
sleep 10

# Check service status
echo -e "${GREEN}📊 Checking service status...${NC}"
docker-compose -f docker-compose.real.yml ps

# Test health endpoint
echo -e "${GREEN}🏥 Testing health endpoint...${NC}"
if curl -f http://localhost:8080/health > /dev/null 2>&1; then
    echo -e "${GREEN}✅ MCP Server is healthy and running!${NC}"
    echo -e "${GREEN}   Health endpoint: http://localhost:8080/health${NC}"
    echo -e "${GREEN}   MCP endpoint: http://localhost:8080/mcp${NC}"
else
    echo -e "${RED}❌ Health check failed. Check logs with:${NC}"
    echo -e "${RED}   docker-compose -f docker-compose.real.yml logs mysql-mcp-server-real${NC}"
fi

echo ""
echo -e "${YELLOW}📝 Useful commands:${NC}"
echo -e "${YELLOW}   View logs: docker-compose -f docker-compose.real.yml logs -f${NC}"
echo -e "${YELLOW}   Stop services: docker-compose -f docker-compose.real.yml down${NC}"
echo -e "${YELLOW}   Restart: docker-compose -f docker-compose.real.yml restart${NC}"
echo ""
echo -e "${GREEN}🎉 Real environment setup complete!${NC}"