#!/bin/bash

# Docker test script for MySQL MCP Server

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${GREEN}🧪 Testing MySQL MCP Server Docker Deployment${NC}"
echo

# Check if services are running
if ! docker-compose ps | grep -q "Up"; then
    echo -e "${RED}❌ Services are not running. Please start them first:${NC}"
    echo "   ./docker/start.sh"
    exit 1
fi

echo -e "${BLUE}Services Status:${NC}"
docker-compose ps
echo

# Test 1: Health Check
echo -e "${YELLOW}Test 1: Health Check${NC}"
HEALTH_RESPONSE=$(curl -s http://localhost:8080/health)
if echo "$HEALTH_RESPONSE" | grep -q '"status":"healthy"'; then
    echo -e "${GREEN}✅ Health check passed${NC}"
    echo "Response: $HEALTH_RESPONSE"
else
    echo -e "${RED}❌ Health check failed${NC}"
    echo "Response: $HEALTH_RESPONSE"
    exit 1
fi
echo

# Test 2: MCP Initialize
echo -e "${YELLOW}Test 2: MCP Initialize${NC}"
INIT_RESPONSE=$(curl -s -X POST http://localhost:8080/mcp \
    -H "Content-Type: application/json" \
    -d '{
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }')

if echo "$INIT_RESPONSE" | grep -q '"protocolVersion"'; then
    echo -e "${GREEN}✅ MCP initialize passed${NC}"
    echo "Response: $INIT_RESPONSE"
else
    echo -e "${RED}❌ MCP initialize failed${NC}"
    echo "Response: $INIT_RESPONSE"
    exit 1
fi
echo

# Test 3: List Tools
echo -e "${YELLOW}Test 3: List Tools${NC}"
TOOLS_RESPONSE=$(curl -s -X POST http://localhost:8080/mcp \
    -H "Content-Type: application/json" \
    -d '{
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    }')

if echo "$TOOLS_RESPONSE" | grep -q '"execute_query"'; then
    echo -e "${GREEN}✅ List tools passed${NC}"
    echo "Response: $TOOLS_RESPONSE"
else
    echo -e "${RED}❌ List tools failed${NC}"
    echo "Response: $TOOLS_RESPONSE"
    exit 1
fi
echo

# Test 4: Test Connection
echo -e "${YELLOW}Test 4: Test Database Connection${NC}"
CONN_RESPONSE=$(curl -s -X POST http://localhost:8080/mcp \
    -H "Content-Type: application/json" \
    -d '{
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "test_connection",
            "arguments": {}
        }
    }')

if echo "$CONN_RESPONSE" | grep -q '"status":"success"'; then
    echo -e "${GREEN}✅ Database connection test passed${NC}"
    echo "Response: $CONN_RESPONSE"
else
    echo -e "${RED}❌ Database connection test failed${NC}"
    echo "Response: $CONN_RESPONSE"
    exit 1
fi
echo

# Test 5: Execute Query
echo -e "${YELLOW}Test 5: Execute Sample Query${NC}"
QUERY_RESPONSE=$(curl -s -X POST http://localhost:8080/mcp \
    -H "Content-Type: application/json" \
    -d '{
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "execute_query",
            "arguments": {
                "sql": "SELECT COUNT(*) as user_count FROM users"
            }
        }
    }')

if echo "$QUERY_RESPONSE" | grep -q '"user_count"'; then
    echo -e "${GREEN}✅ Query execution passed${NC}"
    echo "Response: $QUERY_RESPONSE"
else
    echo -e "${RED}❌ Query execution failed${NC}"
    echo "Response: $QUERY_RESPONSE"
    exit 1
fi
echo

# Test 6: List Databases
echo -e "${YELLOW}Test 6: List Databases${NC}"
DB_RESPONSE=$(curl -s -X POST http://localhost:8080/mcp \
    -H "Content-Type: application/json" \
    -d '{
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "list_databases",
            "arguments": {}
        }
    }')

if echo "$DB_RESPONSE" | grep -q '"mcp_test"'; then
    echo -e "${GREEN}✅ List databases passed${NC}"
    echo "Response: $DB_RESPONSE"
else
    echo -e "${RED}❌ List databases failed${NC}"
    echo "Response: $DB_RESPONSE"
    exit 1
fi
echo

echo -e "${GREEN}🎉 All tests passed successfully!${NC}"
echo
echo -e "${BLUE}Container Information:${NC}"
echo "MCP Server Container:"
docker inspect mysql-mcp-server --format='{{.State.Status}}: {{.Config.Image}}'
echo "MySQL Container:"
docker inspect mysql-mcp-db --format='{{.State.Status}}: {{.Config.Image}}'

echo
echo -e "${YELLOW}Performance Information:${NC}"
echo "Container Resource Usage:"
docker stats --no-stream --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}" mysql-mcp-server mysql-mcp-db

echo
echo -e "${GREEN}✅ Docker deployment test completed successfully!${NC}"