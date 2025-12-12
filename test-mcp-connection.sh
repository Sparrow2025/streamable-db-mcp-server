#!/bin/bash

# Test MCP Connection Script
# This script helps you test if the MySQL MCP Server is working correctly

set -e

echo "🧪 MySQL MCP Server Connection Test"
echo "==================================="

# Check if config.toml exists
if [ ! -f "config.toml" ]; then
    echo "❌ Error: config.toml not found"
    echo "   Please run ./setup-mcp.sh first or copy config.example.toml to config.toml"
    exit 1
fi

echo "✅ Found config.toml"

# Check if the project builds
echo "🔨 Testing build..."
if ! cargo check --quiet; then
    echo "❌ Build check failed"
    echo "   Please fix any compilation errors"
    exit 1
fi

echo "✅ Build check passed"

# Test configuration parsing
echo "🔧 Testing configuration..."
if ! cargo run --bin mysql-mcp-server -- --help > /dev/null 2>&1; then
    echo "⚠️  Could not test configuration directly"
    echo "   This is normal - the server doesn't have a --help flag"
fi

# Try to start the server briefly to test configuration
echo "🚀 Testing server startup..."

# Start server in background
cargo run &
SERVER_PID=$!

# Give it a moment to start
sleep 3

# Check if it's still running (configuration loaded successfully)
if kill -0 $SERVER_PID 2>/dev/null; then
    echo "✅ Server started successfully"
    
    # Try to connect to the server (basic HTTP check)
    if command -v curl >/dev/null 2>&1; then
        echo "🌐 Testing HTTP endpoint..."
        if curl -s -f http://localhost:8080 >/dev/null 2>&1; then
            echo "✅ HTTP endpoint responding"
        else
            echo "⚠️  HTTP endpoint not responding (this may be normal for MCP servers)"
        fi
    fi
    
    # Stop the server
    kill $SERVER_PID
    wait $SERVER_PID 2>/dev/null || true
    echo "🛑 Server stopped"
else
    echo "❌ Server failed to start or crashed"
    echo "   Check your config.toml settings, especially database credentials"
    exit 1
fi

echo ""
echo "🎉 Connection Test Complete!"
echo "=========================="
echo ""
echo "✅ The MySQL MCP Server appears to be configured correctly"
echo ""
echo "Next steps:"
echo "1. 🔗 Make sure your MCP client is configured (see README.md)"
echo "2. 🧪 Test with your MCP client using the 'test_connection' tool"
echo "3. 📊 Try executing a simple query like 'SELECT 1'"
echo ""
echo "🐛 If you have issues:"
echo "   - Check MySQL server is running and accessible"
echo "   - Verify database credentials in config.toml"
echo "   - Run with debug logging: RUST_LOG=debug cargo run"