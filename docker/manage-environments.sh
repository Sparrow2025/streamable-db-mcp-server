#!/bin/bash
# Comprehensive Docker Environment Management Script for MySQL MCP Server

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Default values
ACTION=""
ENVIRONMENT=""
DETACHED=false
REBUILD=false
FOLLOW_LOGS=false

# Available environments
AVAILABLE_ENVS="dev staging production multi"

echo -e "${BLUE}🐳 MySQL MCP Server - Docker Environment Manager${NC}"
echo "=============================================="

# Function to show usage
show_usage() {
    echo "Usage: $0 <action> [environment] [options]"
    echo ""
    echo "Actions:"
    echo "  start       Start environment(s)"
    echo "  stop        Stop environment(s)"
    echo "  restart     Restart environment(s)"
    echo "  status      Show status of environment(s)"
    echo "  logs        Show logs for environment(s)"
    echo "  health      Check health of environment(s)"
    echo "  clean       Clean up environment(s) (removes volumes)"
    echo "  test        Run tests against environment(s)"
    echo ""
    echo "Environments:"
    echo "  dev         Development environment only"
    echo "  staging     Staging environment only"
    echo "  production  Production environment"
    echo "  multi       Multi-environment setup (dev + staging + optional prod-test)"
    echo ""
    echo "Options:"
    echo "  --detached, -d      Run in detached mode (for start action)"
    echo "  --rebuild           Rebuild images before starting"
    echo "  --follow, -f        Follow logs (for logs action)"
    echo "  --with-prod-test    Include prod-test in multi environment"
    echo "  --help, -h          Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 start dev --detached              # Start dev environment in background"
    echo "  $0 start multi --with-prod-test      # Start multi-env with prod-test"
    echo "  $0 logs staging --follow             # Follow staging logs"
    echo "  $0 health multi                      # Check health of multi environment"
    echo "  $0 clean dev                         # Clean up dev environment"
    echo "  $0 test staging                      # Run tests against staging"
}

# Function to get compose file and env file for environment
get_compose_config() {
    local env=$1
    case $env in
        dev)
            COMPOSE_FILE="docker-compose.dev-only.yml"
            ENV_FILE=".env.development"
            ;;
        staging)
            COMPOSE_FILE="docker-compose.staging.yml"
            ENV_FILE=".env.staging"
            ;;
        production)
            COMPOSE_FILE="docker-compose.production.yml"
            ENV_FILE=""  # Production uses secrets
            ;;
        multi)
            COMPOSE_FILE="docker-compose.multi-env.yml"
            ENV_FILE=".env.example"
            ;;
        *)
            echo -e "${RED}❌ Unknown environment: $env${NC}"
            echo "Available environments: $AVAILABLE_ENVS"
            exit 1
            ;;
    esac
}

# Function to build compose command
build_compose_cmd() {
    local env=$1
    get_compose_config "$env"
    
    COMPOSE_CMD="docker-compose -f $COMPOSE_FILE"
    
    if [ -n "$ENV_FILE" ] && [ -f "$ENV_FILE" ]; then
        COMPOSE_CMD="$COMPOSE_CMD --env-file $ENV_FILE"
    fi
    
    if [ "$env" = "multi" ] && [ "$WITH_PROD_TEST" = true ]; then
        COMPOSE_CMD="$COMPOSE_CMD --profile prod-test"
    fi
}

# Function to start environment
start_environment() {
    local env=$1
    echo -e "${BLUE}🚀 Starting $env environment...${NC}"
    
    build_compose_cmd "$env"
    
    if [ "$REBUILD" = true ]; then
        echo -e "${YELLOW}🔨 Rebuilding images...${NC}"
        $COMPOSE_CMD build --no-cache
    fi
    
    if [ "$DETACHED" = true ]; then
        $COMPOSE_CMD up -d
        echo -e "${GREEN}✅ $env environment started in detached mode${NC}"
        
        # Wait for health checks
        echo -e "${BLUE}⏳ Waiting for services to be healthy...${NC}"
        sleep 10
        $COMPOSE_CMD ps
    else
        echo -e "${YELLOW}📝 Starting in foreground mode. Press Ctrl+C to stop.${NC}"
        $COMPOSE_CMD up
    fi
}

# Function to stop environment
stop_environment() {
    local env=$1
    echo -e "${YELLOW}🛑 Stopping $env environment...${NC}"
    
    build_compose_cmd "$env"
    $COMPOSE_CMD down
    
    echo -e "${GREEN}✅ $env environment stopped${NC}"
}

# Function to restart environment
restart_environment() {
    local env=$1
    echo -e "${BLUE}🔄 Restarting $env environment...${NC}"
    
    stop_environment "$env"
    sleep 2
    start_environment "$env"
}

# Function to show status
show_status() {
    local env=$1
    echo -e "${BLUE}📊 Status of $env environment:${NC}"
    
    build_compose_cmd "$env"
    $COMPOSE_CMD ps
    
    echo ""
    echo -e "${BLUE}📈 Resource Usage:${NC}"
    # Get container names for this environment
    CONTAINERS=$($COMPOSE_CMD ps --services | xargs -I {} echo "{}_1" | tr '\n' ' ')
    if [ -n "$CONTAINERS" ]; then
        docker stats --no-stream --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}" $CONTAINERS 2>/dev/null || echo "No running containers found"
    fi
}

# Function to show logs
show_logs() {
    local env=$1
    echo -e "${BLUE}📋 Logs for $env environment:${NC}"
    
    build_compose_cmd "$env"
    
    if [ "$FOLLOW_LOGS" = true ]; then
        $COMPOSE_CMD logs -f
    else
        $COMPOSE_CMD logs --tail=50
    fi
}

# Function to check health
check_health() {
    local env=$1
    echo -e "${BLUE}🏥 Health check for $env environment:${NC}"
    
    build_compose_cmd "$env"
    
    # Check if MCP server is responding
    if curl -f http://localhost:8080/health &>/dev/null; then
        echo -e "${GREEN}✅ MCP Server: Healthy${NC}"
        
        # Test MCP functionality
        echo -e "${BLUE}🧪 Testing MCP functionality...${NC}"
        RESPONSE=$(curl -s -X POST http://localhost:8080/mcp \
            -H 'Content-Type: application/json' \
            -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_environments","arguments":{}}}')
        
        if echo "$RESPONSE" | grep -q '"result"'; then
            echo -e "${GREEN}✅ MCP Tools: Working${NC}"
            echo "Available environments:"
            echo "$RESPONSE" | python3 -m json.tool 2>/dev/null | grep -A 10 '"result"' || echo "  (Raw response: $RESPONSE)"
        else
            echo -e "${YELLOW}⚠️  MCP Tools: Limited functionality${NC}"
        fi
    else
        echo -e "${RED}❌ MCP Server: Not responding${NC}"
    fi
    
    # Check database connections
    echo ""
    echo -e "${BLUE}🗄️  Database Health:${NC}"
    $COMPOSE_CMD ps | grep mysql
}

# Function to clean environment
clean_environment() {
    local env=$1
    echo -e "${YELLOW}🧹 Cleaning $env environment...${NC}"
    echo -e "${RED}⚠️  This will remove all data volumes!${NC}"
    
    read -p "Are you sure? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        build_compose_cmd "$env"
        $COMPOSE_CMD down -v --remove-orphans
        echo -e "${GREEN}✅ $env environment cleaned${NC}"
    else
        echo -e "${BLUE}ℹ️  Clean operation cancelled${NC}"
    fi
}

# Function to test environment
test_environment() {
    local env=$1
    echo -e "${BLUE}🧪 Testing $env environment...${NC}"
    
    # Check if environment is running
    build_compose_cmd "$env"
    if ! $COMPOSE_CMD ps | grep -q "Up"; then
        echo -e "${RED}❌ Environment is not running. Please start it first.${NC}"
        exit 1
    fi
    
    # Run comprehensive tests
    echo -e "${YELLOW}Running comprehensive tests...${NC}"
    
    # Test 1: Health check
    echo -n "Health check... "
    if curl -f http://localhost:8080/health &>/dev/null; then
        echo -e "${GREEN}✅${NC}"
    else
        echo -e "${RED}❌${NC}"
        return 1
    fi
    
    # Test 2: MCP Initialize
    echo -n "MCP Initialize... "
    INIT_RESPONSE=$(curl -s -X POST http://localhost:8080/mcp \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}')
    
    if echo "$INIT_RESPONSE" | grep -q '"protocolVersion"'; then
        echo -e "${GREEN}✅${NC}"
    else
        echo -e "${RED}❌${NC}"
        return 1
    fi
    
    # Test 3: List environments
    echo -n "List environments... "
    ENV_RESPONSE=$(curl -s -X POST http://localhost:8080/mcp \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_environments","arguments":{}}}')
    
    if echo "$ENV_RESPONSE" | grep -q '"result"'; then
        echo -e "${GREEN}✅${NC}"
    else
        echo -e "${RED}❌${NC}"
        return 1
    fi
    
    echo -e "${GREEN}🎉 All tests passed!${NC}"
}

# Parse command line arguments
WITH_PROD_TEST=false

while [[ $# -gt 0 ]]; do
    case $1 in
        start|stop|restart|status|logs|health|clean|test)
            if [ -z "$ACTION" ]; then
                ACTION=$1
            else
                echo -e "${RED}❌ Multiple actions specified${NC}"
                exit 1
            fi
            shift
            ;;
        dev|staging|production|multi)
            if [ -z "$ENVIRONMENT" ]; then
                ENVIRONMENT=$1
            else
                echo -e "${RED}❌ Multiple environments specified${NC}"
                exit 1
            fi
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
        --follow|-f)
            FOLLOW_LOGS=true
            shift
            ;;
        --with-prod-test)
            WITH_PROD_TEST=true
            shift
            ;;
        --help|-h)
            show_usage
            exit 0
            ;;
        *)
            echo -e "${RED}❌ Unknown option: $1${NC}"
            show_usage
            exit 1
            ;;
    esac
done

# Validate required arguments
if [ -z "$ACTION" ]; then
    echo -e "${RED}❌ No action specified${NC}"
    show_usage
    exit 1
fi

if [ -z "$ENVIRONMENT" ]; then
    echo -e "${RED}❌ No environment specified${NC}"
    show_usage
    exit 1
fi

# Change to docker directory
cd "$(dirname "$0")"

# Execute action
case $ACTION in
    start)
        start_environment "$ENVIRONMENT"
        ;;
    stop)
        stop_environment "$ENVIRONMENT"
        ;;
    restart)
        restart_environment "$ENVIRONMENT"
        ;;
    status)
        show_status "$ENVIRONMENT"
        ;;
    logs)
        show_logs "$ENVIRONMENT"
        ;;
    health)
        check_health "$ENVIRONMENT"
        ;;
    clean)
        clean_environment "$ENVIRONMENT"
        ;;
    test)
        test_environment "$ENVIRONMENT"
        ;;
    *)
        echo -e "${RED}❌ Unknown action: $ACTION${NC}"
        show_usage
        exit 1
        ;;
esac