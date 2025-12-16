#!/bin/bash
# Comprehensive Health Check Script for MySQL MCP Server Docker Deployments

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
MCP_SERVER_URL="http://localhost:8080"
TIMEOUT=10
VERBOSE=false

# Function to show usage
show_usage() {
    echo "Usage: $0 [options]"
    echo ""
    echo "Options:"
    echo "  --url URL           MCP server URL (default: http://localhost:8080)"
    echo "  --timeout SECONDS   Timeout for requests (default: 10)"
    echo "  --verbose, -v       Show detailed output"
    echo "  --help, -h          Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                              # Basic health check"
    echo "  $0 --verbose                    # Detailed health check"
    echo "  $0 --url http://server:8080     # Check remote server"
}

# Function to log messages
log() {
    if [ "$VERBOSE" = true ]; then
        echo -e "$1"
    fi
}

# Function to check HTTP endpoint
check_http() {
    local url="$1"
    local expected_pattern="$2"
    local description="$3"
    
    log "${BLUE}Checking $description: $url${NC}"
    
    local response
    response=$(curl -s -f --max-time "$TIMEOUT" "$url" 2>&1)
    local exit_code=$?
    
    log "${BLUE}Response: $response${NC}"
    
    if [ $exit_code -eq 0 ] && echo "$response" | grep -q "$expected_pattern"; then
        echo -e "${GREEN}✅ $description: OK${NC}"
        return 0
    else
        echo -e "${RED}❌ $description: FAILED${NC}"
        if [ "$VERBOSE" = true ]; then
            echo -e "${RED}Expected pattern: $expected_pattern${NC}"
            echo -e "${RED}Actual response: $response${NC}"
        fi
        return 1
    fi
}

# Function to check MCP endpoint
check_mcp() {
    local method="$1"
    local params="$2"
    local expected_pattern="$3"
    local description="$4"
    
    log "${BLUE}Checking $description${NC}"
    
    local payload="{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$method\""
    if [ -n "$params" ]; then
        payload="$payload,\"params\":$params"
    fi
    payload="$payload}"
    
    log "${BLUE}Payload: $payload${NC}"
    
    local response
    response=$(curl -s -f --max-time "$TIMEOUT" \
        -X POST "$MCP_SERVER_URL/mcp" \
        -H "Content-Type: application/json" \
        -d "$payload" 2>&1)
    local exit_code=$?
    
    log "${BLUE}Response: $response${NC}"
    
    if [ $exit_code -eq 0 ] && echo "$response" | grep -q "$expected_pattern"; then
        echo -e "${GREEN}✅ $description: OK${NC}"
        return 0
    else
        echo -e "${RED}❌ $description: FAILED${NC}"
        if [ "$VERBOSE" = true ]; then
            echo -e "${RED}Expected pattern: $expected_pattern${NC}"
            echo -e "${RED}Actual response: $response${NC}"
        fi
        return 1
    fi
}

# Function to check database connectivity through MCP
check_database_connectivity() {
    log "${BLUE}Checking database connectivity through MCP${NC}"
    
    # Try to list environments
    if check_mcp "tools/call" '{"name":"list_environments","arguments":{}}' '"result"' "List Environments"; then
        # Try to test connection
        check_mcp "tools/call" '{"name":"test_connection","arguments":{}}' '"status":"success"' "Database Connection Test"
    else
        return 1
    fi
}

# Function to perform comprehensive health check
comprehensive_health_check() {
    echo -e "${BLUE}🏥 MySQL MCP Server Health Check${NC}"
    echo "=================================="
    
    local failed=0
    
    # Basic HTTP health check
    check_http "$MCP_SERVER_URL/health" '"status":"healthy"' "HTTP Health Endpoint" || ((failed++))
    
    # MCP Protocol checks
    check_mcp "initialize" '{}' '"protocolVersion"' "MCP Initialize" || ((failed++))
    check_mcp "tools/list" '' '"execute_query"' "MCP Tools List" || ((failed++))
    
    # Database connectivity
    check_database_connectivity || ((failed++))
    
    # Environment-specific checks (if multi-environment)
    if check_mcp "tools/call" '{"name":"list_environments","arguments":{}}' '"result"' "Environment Detection" >/dev/null 2>&1; then
        log "${BLUE}Multi-environment setup detected, running additional checks${NC}"
        
        # Try environment-specific query
        check_mcp "tools/call" '{"name":"execute_query","arguments":{"sql":"SELECT 1 as health_check"}}' '"health_check"' "Environment Query Test" || ((failed++))
    fi
    
    echo ""
    echo "=================================="
    
    if [ $failed -eq 0 ]; then
        echo -e "${GREEN}🎉 All health checks passed!${NC}"
        echo -e "${GREEN}✅ MySQL MCP Server is healthy and operational${NC}"
        exit 0
    else
        echo -e "${RED}❌ $failed health check(s) failed${NC}"
        echo -e "${RED}🚨 MySQL MCP Server has health issues${NC}"
        exit 1
    fi
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --url)
            MCP_SERVER_URL="$2"
            shift 2
            ;;
        --timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        --verbose|-v)
            VERBOSE=true
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

# Run comprehensive health check
comprehensive_health_check