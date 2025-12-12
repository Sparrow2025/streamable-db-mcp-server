#!/bin/bash

# Docker start script for MySQL MCP Server

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default mode
MODE="production"

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --dev|--development)
            MODE="development"
            shift
            ;;
        --prod|--production)
            MODE="production"
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [--dev|--development] [--prod|--production]"
            echo "  --dev, --development    Start in development mode"
            echo "  --prod, --production    Start in production mode (default)"
            exit 0
            ;;
        *)
            echo "Unknown option $1"
            exit 1
            ;;
    esac
done

echo -e "${GREEN}🚀 Starting MySQL MCP Server in ${MODE} mode${NC}"
echo

if [ "$MODE" = "development" ]; then
    echo -e "${YELLOW}Development mode features:${NC}"
    echo "- Hot reload with volume mounts"
    echo "- Debug logging enabled"
    echo "- Cargo cache persistence"
    echo
    
    # Check if development images exist
    if ! docker images | grep -q "mysql-mcp-server.*dev"; then
        echo -e "${YELLOW}Building development image...${NC}"
        docker build -f Dockerfile.dev -t mysql-mcp-server:dev .
    fi
    
    docker-compose -f docker-compose.dev.yml up -d
    COMPOSE_FILE="docker-compose.dev.yml"
else
    echo -e "${YELLOW}Production mode features:${NC}"
    echo "- Optimized release build"
    echo "- Minimal container size"
    echo "- Production logging"
    echo
    
    # Check if production images exist
    if ! docker images | grep -q "mysql-mcp-server.*latest"; then
        echo -e "${YELLOW}Building production image...${NC}"
        docker build -t mysql-mcp-server:latest .
    fi
    
    docker-compose up -d
    COMPOSE_FILE="docker-compose.yml"
fi

echo
echo -e "${GREEN}✅ Services started successfully!${NC}"
echo

# Wait for services to be ready
echo -e "${BLUE}Waiting for services to be ready...${NC}"
sleep 5

# Check service status
echo -e "${YELLOW}Service Status:${NC}"
docker-compose -f "$COMPOSE_FILE" ps

echo
echo -e "${YELLOW}Useful commands:${NC}"
echo "📋 View logs:           docker-compose -f $COMPOSE_FILE logs -f"
echo "🔍 Check health:       curl http://localhost:8080/health"
echo "🧪 Test MCP:           curl -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}'"
echo "🛑 Stop services:      docker-compose -f $COMPOSE_FILE down"
echo "🗑️  Clean up:          docker-compose -f $COMPOSE_FILE down -v"

echo
echo -e "${GREEN}🌐 Access URLs:${NC}"
echo "MCP Server:     http://localhost:8080/mcp"
echo "Health Check:   http://localhost:8080/health"
echo "Streaming:      http://localhost:8080/stream/query"
echo "MySQL DB:       localhost:3306 (user: mcp_user, password: mcp_password)"