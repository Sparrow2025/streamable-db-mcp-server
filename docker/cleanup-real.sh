#!/bin/bash

# Cleanup script for real AWS RDS Aurora environment

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🧹 Cleaning up real environment Docker resources...${NC}"

# Stop and remove containers
if docker-compose -f docker-compose.real.yml ps -q > /dev/null 2>&1; then
    echo -e "${GREEN}🛑 Stopping services...${NC}"
    docker-compose -f docker-compose.real.yml down
else
    echo -e "${YELLOW}ℹ️  No running services found${NC}"
fi

# Parse command line arguments
FULL_CLEANUP=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --full)
            FULL_CLEANUP=true
            shift
            ;;
        *)
            echo -e "${RED}❌ Unknown option: $1${NC}"
            echo "Usage: $0 [--full]"
            echo "  --full: Remove images and networks (complete cleanup)"
            exit 1
            ;;
    esac
done

if [ "$FULL_CLEANUP" = true ]; then
    echo -e "${YELLOW}🚨 Performing full cleanup (removing images and networks)...${NC}"
    
    # Remove images
    if docker images mysql-mcp-server-real -q > /dev/null 2>&1; then
        echo -e "${GREEN}🗑️  Removing images...${NC}"
        docker rmi $(docker images mysql-mcp-server-real -q) 2>/dev/null || true
    fi
    
    # Remove networks
    if docker network ls | grep -q mcp-real-network; then
        echo -e "${GREEN}🌐 Removing networks...${NC}"
        docker network rm mcp-real-network 2>/dev/null || true
    fi
    
    # Clean up unused resources
    echo -e "${GREEN}🧽 Cleaning up unused Docker resources...${NC}"
    docker system prune -f
fi

echo -e "${GREEN}✅ Cleanup complete!${NC}"