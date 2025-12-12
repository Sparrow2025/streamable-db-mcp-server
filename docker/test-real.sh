#!/bin/bash

# Test script for real AWS RDS Aurora environment
# Tests MCP server functionality against real database

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
MCP_SERVER_URL="http://localhost:8080"
HEALTH_ENDPOINT="$MCP_SERVER_URL/health"
MCP_ENDPOINT="$MCP_SERVER_URL/mcp"

echo -e "${BLUE}🧪 Testing MCP Server with Real AWS RDS Aurora Database${NC}"
echo -e "${YELLOW}⚠️  WARNING: This will test against the actual UAT database!${NC}"
echo ""

# Function to make HTTP requests with error handling
make_request() {
    local url="$1"
    local method="${2:-GET}"
    local data="$3"
    local description="$4"
    
    echo -e "${BLUE}🔍 Testing: $description${NC}"
    
    if [ -n "$data" ]; then
        response=$(curl -s -w "\n%{http_code}" -X "$method" \
            -H "Content-Type: application/json" \
            -d "$data" \
            "$url" 2>/dev/null)
    else
        response=$(curl -s -w "\n%{http_code}" -X "$method" "$url" 2>/dev/null)
    fi
    
    # Extract body and status code
    body=$(echo "$response" | head -n -1)
    status_code=$(echo "$response" | tail -n 1)
    
    if [ "$status_code" -eq 200 ]; then
        echo -e "${GREEN}✅ Success (HTTP $status_code)${NC}"
        if [ -n "$body" ] && [ "$body" != "null" ]; then
            echo -e "${GREEN}   Response: $body${NC}"
        fi
        return 0
    else
        echo -e "${RED}❌ Failed (HTTP $status_code)${NC}"
        if [ -n "$body" ]; then
            echo -e "${RED}   Error: $body${NC}"
        fi
        return 1
    fi
}

# Test 1: Health Check
echo -e "${YELLOW}=== Test 1: Health Check ===${NC}"
if make_request "$HEALTH_ENDPOINT" "GET" "" "Health endpoint"; then
    echo -e "${GREEN}✅ Health check passed${NC}"
else
    echo -e "${RED}❌ Health check failed - server may not be running${NC}"
    echo -e "${RED}   Try: docker-compose -f docker-compose.real.yml logs${NC}"
    exit 1
fi
echo ""

# Test 2: MCP Protocol Initialization
echo -e "${YELLOW}=== Test 2: MCP Protocol Initialization ===${NC}"
init_request='{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "roots": {
                "listChanged": true
            },
            "sampling": {}
        },
        "clientInfo": {
            "name": "test-client",
            "version": "1.0.0"
        }
    }
}'

if make_request "$MCP_ENDPOINT" "POST" "$init_request" "MCP initialization"; then
    echo -e "${GREEN}✅ MCP initialization successful${NC}"
else
    echo -e "${RED}❌ MCP initialization failed${NC}"
    exit 1
fi
echo ""

# Test 3: List Available Tools
echo -e "${YELLOW}=== Test 3: List Available Tools ===${NC}"
tools_request='{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/list",
    "params": {}
}'

if make_request "$MCP_ENDPOINT" "POST" "$tools_request" "Tools listing"; then
    echo -e "${GREEN}✅ Tools listing successful${NC}"
else
    echo -e "${RED}❌ Tools listing failed${NC}"
fi
echo ""

# Test 4: Test Database Connection (List Databases)
echo -e "${YELLOW}=== Test 4: Database Connection Test ===${NC}"
db_list_request='{
    "jsonrpc": "2.0",
    "id": 3,
    "method": "tools/call",
    "params": {
        "name": "list_databases",
        "arguments": {}
    }
}'

if make_request "$MCP_ENDPOINT" "POST" "$db_list_request" "Database connection"; then
    echo -e "${GREEN}✅ Database connection successful${NC}"
else
    echo -e "${RED}❌ Database connection failed${NC}"
    echo -e "${RED}   Check database credentials and network connectivity${NC}"
fi
echo ""

# Test 5: Execute Simple Query
echo -e "${YELLOW}=== Test 5: Execute Simple Query ===${NC}"
query_request='{
    "jsonrpc": "2.0",
    "id": 4,
    "method": "tools/call",
    "params": {
        "name": "execute_query",
        "arguments": {
            "query": "SELECT VERSION() as mysql_version, NOW() as current_time"
        }
    }
}'

if make_request "$MCP_ENDPOINT" "POST" "$query_request" "Simple query execution"; then
    echo -e "${GREEN}✅ Query execution successful${NC}"
else
    echo -e "${RED}❌ Query execution failed${NC}"
fi
echo ""

# Test 6: List Tables in information_schema
echo -e "${YELLOW}=== Test 6: List Tables in information_schema ===${NC}"
tables_request='{
    "jsonrpc": "2.0",
    "id": 5,
    "method": "tools/call",
    "params": {
        "name": "list_tables",
        "arguments": {
            "database": "information_schema"
        }
    }
}'

if make_request "$MCP_ENDPOINT" "POST" "$tables_request" "List tables"; then
    echo -e "${GREEN}✅ Table listing successful${NC}"
else
    echo -e "${RED}❌ Table listing failed${NC}"
fi
echo ""

# Test 7: Container Resource Usage
echo -e "${YELLOW}=== Test 7: Container Resource Usage ===${NC}"
echo -e "${BLUE}🔍 Checking container resource usage...${NC}"
if docker stats --no-stream mysql-mcp-server-real > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Container is running and responsive${NC}"
    docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}" mysql-mcp-server-real
else
    echo -e "${RED}❌ Container resource check failed${NC}"
fi
echo ""

# Summary
echo -e "${BLUE}=== Test Summary ===${NC}"
echo -e "${GREEN}🎉 Real environment testing completed!${NC}"
echo ""
echo -e "${YELLOW}📊 Connection Details:${NC}"
echo -e "${YELLOW}   Database Host: dcs-uat-rds-aurora-cluster.cluster-czcmoige2cq2.ap-southeast-1.rds.amazonaws.com${NC}"
echo -e "${YELLOW}   Database: information_schema${NC}"
echo -e "${YELLOW}   MCP Server: $MCP_SERVER_URL${NC}"
echo ""
echo -e "${YELLOW}📝 Useful commands:${NC}"
echo -e "${YELLOW}   View logs: docker-compose -f docker-compose.real.yml logs -f${NC}"
echo -e "${YELLOW}   Stop server: docker-compose -f docker-compose.real.yml down${NC}"
echo -e "${YELLOW}   Health check: curl $HEALTH_ENDPOINT${NC}"