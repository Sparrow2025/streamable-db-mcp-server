#!/bin/bash
# Start MySQL MCP Server with Multi-Environment Docker Setup

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Starting MySQL MCP Server Multi-Environment Setup${NC}"
echo "=================================================="

# Check if Docker and Docker Compose are available
if ! command -v docker &> /dev/null; then
    echo -e "${RED}❌ Docker is not installed or not in PATH${NC}"
    exit 1
fi

if ! command -v docker-compose &> /dev/null; then
    echo -e "${RED}❌ Docker Compose is not installed or not in PATH${NC}"
    exit 1
fi

# Change to the docker directory
cd "$(dirname "$0")"

# Check if configuration file exists
if [ ! -f "config.multi-env.docker.toml" ]; then
    echo -e "${RED}❌ Configuration file not found: config.multi-env.docker.toml${NC}"
    exit 1
fi

# Parse command line arguments
INCLUDE_PROD_TEST=false
DETACHED=false
REBUILD=false
ENV_FILE=""
COMPOSE_FILE="docker-compose.multi-env.yml"

while [[ $# -gt 0 ]]; do
    case $1 in
        --with-prod-test)
            INCLUDE_PROD_TEST=true
            shift
            ;;
        --detached|-d)
            DETACHED=true
            shift
            ;;
        --rebuild)
            REBUILD=true
            shift
            ;;
        --env-file)
            ENV_FILE="$2"
            shift 2
            ;;
        --compose-file)
            COMPOSE_FILE="$2"
            shift 2
            ;;
        --dev-only)
            COMPOSE_FILE="docker-compose.dev-only.yml"
            ENV_FILE=".env.development"
            shift
            ;;
        --staging)
            COMPOSE_FILE="docker-compose.staging.yml"
            ENV_FILE=".env.staging"
            shift
            ;;
        --production)
            COMPOSE_FILE="docker-compose.production.yml"
            echo -e "${YELLOW}⚠️  Production mode requires Docker secrets setup${NC}"
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --with-prod-test       Include production test environment (multi-env only)"
            echo "  --detached, -d         Run in detached mode"
            echo "  --rebuild              Rebuild images before starting"
            echo "  --env-file FILE        Use specific environment file"
            echo "  --compose-file FILE    Use specific Docker Compose file"
            echo "  --dev-only             Use development-only setup"
            echo "  --staging              Use staging environment setup"
            echo "  --production           Use production environment setup"
            echo "  --help, -h             Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0 --dev-only                    # Start development environment only"
            echo "  $0 --staging --detached          # Start staging environment in background"
            echo "  $0 --with-prod-test              # Start multi-env with production test"
            echo "  $0 --env-file .env.custom        # Use custom environment file"
            exit 0
            ;;
        *)
            echo -e "${RED}❌ Unknown option: $1${NC}"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Build Docker Compose command
COMPOSE_CMD="docker-compose -f $COMPOSE_FILE"

# Add environment file if specified
if [ -n "$ENV_FILE" ]; then
    if [ -f "$ENV_FILE" ]; then
        echo -e "${BLUE}📄 Using environment file: $ENV_FILE${NC}"
        COMPOSE_CMD="$COMPOSE_CMD --env-file $ENV_FILE"
    else
        echo -e "${RED}❌ Environment file not found: $ENV_FILE${NC}"
        exit 1
    fi
fi

# Add production test profile if requested (only for multi-env)
if [ "$INCLUDE_PROD_TEST" = true ] && [ "$COMPOSE_FILE" = "docker-compose.multi-env.yml" ]; then
    echo -e "${YELLOW}⚠️  Including production test environment${NC}"
    COMPOSE_CMD="$COMPOSE_CMD --profile prod-test"
elif [ "$INCLUDE_PROD_TEST" = true ]; then
    echo -e "${YELLOW}⚠️  --with-prod-test only works with multi-environment setup${NC}"
fi

echo -e "${BLUE}🐳 Using Docker Compose file: $COMPOSE_FILE${NC}"

# Rebuild if requested
if [ "$REBUILD" = true ]; then
    echo -e "${BLUE}🔨 Rebuilding Docker images...${NC}"
    $COMPOSE_CMD build --no-cache
fi

# Start services
echo -e "${BLUE}🐳 Starting Docker services...${NC}"

if [ "$DETACHED" = true ]; then
    $COMPOSE_CMD up -d
    
    echo -e "${GREEN}✅ Services started in detached mode${NC}"
    echo ""
    echo "Services status:"
    $COMPOSE_CMD ps
    
    echo ""
    echo -e "${BLUE}📋 Useful commands:${NC}"
    echo "  View logs:           docker-compose -f docker-compose.multi-env.yml logs -f"
    echo "  Stop services:       docker-compose -f docker-compose.multi-env.yml down"
    echo "  Check health:        curl http://localhost:8080/health"
    echo "  List environments:   curl -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"list_environments\",\"arguments\":{}}}'"
else
    echo -e "${YELLOW}📝 Starting in foreground mode. Press Ctrl+C to stop.${NC}"
    $COMPOSE_CMD up
fi

# Wait for services to be healthy (only in detached mode)
if [ "$DETACHED" = true ]; then
    echo ""
    echo -e "${BLUE}⏳ Waiting for services to be healthy...${NC}"
    
    # Wait for MySQL services based on compose file
    if [ "$COMPOSE_FILE" = "docker-compose.multi-env.yml" ]; then
        MYSQL_SERVICES="mysql-dev mysql-staging"
        if [ "$INCLUDE_PROD_TEST" = true ]; then
            MYSQL_SERVICES="$MYSQL_SERVICES mysql-prod-test"
        fi
    elif [ "$COMPOSE_FILE" = "docker-compose.dev-only.yml" ]; then
        MYSQL_SERVICES="mysql-dev"
    elif [ "$COMPOSE_FILE" = "docker-compose.staging.yml" ]; then
        MYSQL_SERVICES="mysql-staging"
    elif [ "$COMPOSE_FILE" = "docker-compose.production.yml" ]; then
        MYSQL_SERVICES="mysql-prod"
    else
        MYSQL_SERVICES="mysql-dev"  # fallback
    fi
    
    for service in $MYSQL_SERVICES; do
        echo -n "  Waiting for $service... "
        timeout=60
        while [ $timeout -gt 0 ]; do
            if $COMPOSE_CMD exec -T $service mysqladmin ping -h localhost --silent 2>/dev/null; then
                echo -e "${GREEN}✅${NC}"
                break
            fi
            sleep 2
            timeout=$((timeout - 2))
        done
        
        if [ $timeout -le 0 ]; then
            echo -e "${RED}❌ Timeout${NC}"
        fi
    done
    
    # Wait for MCP server
    echo -n "  Waiting for MCP server... "
    timeout=60
    while [ $timeout -gt 0 ]; do
        if curl -f http://localhost:8080/health &>/dev/null; then
            echo -e "${GREEN}✅${NC}"
            break
        fi
        sleep 2
        timeout=$((timeout - 2))
    done
    
    if [ $timeout -le 0 ]; then
        echo -e "${RED}❌ Timeout${NC}"
    fi
    
    echo ""
    echo -e "${GREEN}🎉 Multi-environment setup is ready!${NC}"
    echo ""
    echo -e "${BLUE}📊 Environment Status:${NC}"
    
    # Test the list_environments endpoint
    if command -v curl &> /dev/null; then
        curl -s -X POST http://localhost:8080/mcp \
            -H 'Content-Type: application/json' \
            -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_environments","arguments":{}}}' \
            | python3 -m json.tool 2>/dev/null || echo "  (Use MCP client to check environment status)"
    fi
    
    echo ""
    echo -e "${BLUE}🔗 Access Points:${NC}"
    echo "  MCP Server:          http://localhost:8080/mcp"
    echo "  Health Check:        http://localhost:8080/health"
    
    # Show database access points based on compose file
    if [ "$COMPOSE_FILE" = "docker-compose.multi-env.yml" ]; then
        echo "  Dev MySQL:           localhost:3306"
        echo "  Staging MySQL:       localhost:3307"
        if [ "$INCLUDE_PROD_TEST" = true ]; then
            echo "  Prod Test MySQL:     localhost:3308"
        fi
    elif [ "$COMPOSE_FILE" = "docker-compose.dev-only.yml" ]; then
        echo "  Dev MySQL:           localhost:3306"
    elif [ "$COMPOSE_FILE" = "docker-compose.staging.yml" ]; then
        echo "  Staging MySQL:       localhost:3307"
    elif [ "$COMPOSE_FILE" = "docker-compose.production.yml" ]; then
        echo "  Production MySQL:    localhost:3306"
    fi
fi