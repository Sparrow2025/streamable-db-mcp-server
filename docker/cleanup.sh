#!/bin/bash

# Docker cleanup script for MySQL MCP Server

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}🧹 Cleaning up MySQL MCP Server Docker resources${NC}"
echo

# Parse command line arguments
FULL_CLEANUP=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --full)
            FULL_CLEANUP=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [--full]"
            echo "  --full    Perform full cleanup including images and volumes"
            exit 0
            ;;
        *)
            echo "Unknown option $1"
            exit 1
            ;;
    esac
done

# Stop and remove containers
echo -e "${YELLOW}Stopping containers...${NC}"
docker-compose down 2>/dev/null || true
docker-compose -f docker-compose.dev.yml down 2>/dev/null || true

if [ "$FULL_CLEANUP" = true ]; then
    echo -e "${YELLOW}Performing full cleanup...${NC}"
    
    # Remove volumes
    echo "Removing volumes..."
    docker-compose down -v 2>/dev/null || true
    docker-compose -f docker-compose.dev.yml down -v 2>/dev/null || true
    
    # Remove images
    echo "Removing images..."
    docker rmi mysql-mcp-server:latest 2>/dev/null || true
    docker rmi mysql-mcp-server:dev 2>/dev/null || true
    
    # Remove unused networks
    echo "Removing unused networks..."
    docker network prune -f
    
    # Remove unused volumes
    echo "Removing unused volumes..."
    docker volume prune -f
    
    echo -e "${GREEN}✅ Full cleanup completed${NC}"
else
    echo -e "${GREEN}✅ Basic cleanup completed${NC}"
    echo -e "${YELLOW}Note: Use --full flag to remove images and volumes${NC}"
fi

echo
echo -e "${YELLOW}Remaining Docker resources:${NC}"
echo "Images:"
docker images | grep mysql-mcp-server || echo "No mysql-mcp-server images found"
echo
echo "Volumes:"
docker volume ls | grep mysql || echo "No mysql volumes found"
echo
echo "Networks:"
docker network ls | grep mcp || echo "No mcp networks found"