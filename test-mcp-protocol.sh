#!/bin/bash

# Test MCP Protocol Implementation
echo "Testing MCP Protocol Implementation..."

BASE_URL="http://localhost:8080/mcp"

echo "1. Testing OPTIONS request (CORS preflight)..."
curl -X OPTIONS "$BASE_URL" -v -s -o /dev/null -w "HTTP Status: %{http_code}\n"

echo -e "\n2. Testing GET request..."
curl -X GET "$BASE_URL" -s | jq .

echo -e "\n3. Testing initialize method..."
curl -X POST "$BASE_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  -s | jq .

echo -e "\n4. Testing tools/list method..."
curl -X POST "$BASE_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  -s | jq .

echo -e "\n5. Testing tools/call method (test_connection)..."
curl -X POST "$BASE_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"test_connection","arguments":{}}}' \
  -s | jq .

echo -e "\n6. Testing tools/call method (list_databases)..."
curl -X POST "$BASE_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"list_databases","arguments":{}}}' \
  -s | jq .

echo -e "\nMCP Protocol test completed!"