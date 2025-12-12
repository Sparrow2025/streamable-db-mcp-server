#!/bin/bash

# Test HTTP Endpoints Script
# This script tests the MySQL MCP Server HTTP endpoints

set -e

echo "🌐 MySQL MCP Server HTTP Endpoint Test"
echo "======================================"

# Check if server is running
SERVER_URL="http://localhost:8080"
MCP_ENDPOINT="$SERVER_URL/mcp"

echo "🔍 Checking if server is running at $SERVER_URL..."

if ! curl -s -f "$SERVER_URL" >/dev/null 2>&1; then
    echo "❌ Server is not running at $SERVER_URL"
    echo ""
    echo "Please start the server first:"
    echo "  cargo run --release"
    echo ""
    echo "Then run this test script again."
    exit 1
fi

echo "✅ Server is responding"

# Test 1: Initialize
echo ""
echo "🔧 Test 1: Initialize MCP Protocol"
echo "=================================="

INIT_RESPONSE=$(curl -s -X POST "$MCP_ENDPOINT" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize"
  }')

echo "Response: $INIT_RESPONSE"

if echo "$INIT_RESPONSE" | grep -q "protocolVersion"; then
    echo "✅ Initialize test passed"
else
    echo "❌ Initialize test failed"
    exit 1
fi

# Test 2: List Tools
echo ""
echo "🛠️  Test 2: List Available Tools"
echo "==============================="

TOOLS_RESPONSE=$(curl -s -X POST "$MCP_ENDPOINT" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/list"
  }')

echo "Response: $TOOLS_RESPONSE"

if echo "$TOOLS_RESPONSE" | grep -q "execute_query"; then
    echo "✅ List tools test passed"
else
    echo "❌ List tools test failed"
    exit 1
fi

# Test 3: Test Connection
echo ""
echo "🔌 Test 3: Test Database Connection"
echo "=================================="

CONNECTION_RESPONSE=$(curl -s -X POST "$MCP_ENDPOINT" \
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

echo "Response: $CONNECTION_RESPONSE"

if echo "$CONNECTION_RESPONSE" | grep -q "success"; then
    echo "✅ Database connection test passed"
else
    echo "⚠️  Database connection test failed - check your database configuration"
    echo "   This is expected if you haven't configured a real database yet"
fi

# Test 4: Execute Simple Query
echo ""
echo "📊 Test 4: Execute Simple Query"
echo "==============================="

QUERY_RESPONSE=$(curl -s -X POST "$MCP_ENDPOINT" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 4,
    "method": "tools/call",
    "params": {
      "name": "execute_query",
      "arguments": {
        "sql": "SELECT 1 as test_column"
      }
    }
  }')

echo "Response: $QUERY_RESPONSE"

if echo "$QUERY_RESPONSE" | grep -q "test_column"; then
    echo "✅ Simple query test passed"
else
    echo "⚠️  Simple query test failed - check your database configuration"
    echo "   This is expected if you haven't configured a real database yet"
fi

# Test 5: Test Streaming Endpoint
echo ""
echo "🌊 Test 5: Test Streaming Endpoint"
echo "================================="

STREAM_ENDPOINT="$SERVER_URL/stream/query"

echo "Testing streaming endpoint at $STREAM_ENDPOINT..."

STREAM_RESPONSE=$(curl -s -X POST "$STREAM_ENDPOINT" \
  -H "Content-Type: application/json" \
  -d '{
    "sql": "SELECT 1 as stream_test",
    "stream_results": true
  }' || echo "STREAM_FAILED")

if [ "$STREAM_RESPONSE" != "STREAM_FAILED" ]; then
    echo "✅ Streaming endpoint is accessible"
    echo "Response preview: $(echo "$STREAM_RESPONSE" | head -c 200)..."
else
    echo "⚠️  Streaming endpoint test failed - check your database configuration"
fi

echo ""
echo "🎉 HTTP Endpoint Tests Complete!"
echo "==============================="
echo ""
echo "Summary:"
echo "✅ Server is running and responding"
echo "✅ MCP protocol endpoints are working"
echo "✅ JSON-RPC communication is functional"
echo ""
echo "Next steps:"
echo "1. 📝 Configure your database credentials in config.toml"
echo "2. 🔄 Restart the server to apply database configuration"
echo "3. 🔗 Configure your MCP client to use: $MCP_ENDPOINT"
echo "4. 🧪 Test with your MCP client using the available tools"
echo ""
echo "📚 For MCP client configuration, see:"
echo "   - README.md"
echo "   - mcp-config-examples.md"