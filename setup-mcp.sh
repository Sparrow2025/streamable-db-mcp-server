#!/bin/bash

# MySQL MCP Server Setup Script
# This script helps you configure the MySQL MCP Server for use with MCP clients

set -e

echo "🚀 MySQL MCP Server Setup"
echo "=========================="

# Get current directory
CURRENT_DIR=$(pwd)
echo "📁 Current directory: $CURRENT_DIR"

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -f "src/main.rs" ]; then
    echo "❌ Error: This doesn't appear to be the MySQL MCP Server directory"
    echo "   Please run this script from the mysql-mcp-server project root"
    exit 1
fi

# Build the project
echo "🔨 Building the project..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "❌ Build failed. Please fix any compilation errors and try again."
    exit 1
fi

echo "✅ Build successful!"

# Check if config.toml exists
if [ ! -f "config.toml" ]; then
    echo "📝 Creating config.toml from example..."
    cp config.example.toml config.toml
    echo "⚠️  Please edit config.toml with your database credentials"
else
    echo "✅ config.toml already exists"
fi

# Detect MCP client and provide configuration
echo ""
echo "🔧 MCP Client Configuration"
echo "=========================="

# Check for Kiro IDE
KIRO_CONFIG_DIR="$HOME/.kiro/settings"
if [ -d "$KIRO_CONFIG_DIR" ]; then
    echo "🎯 Kiro IDE detected!"
    
    MCP_CONFIG_FILE="$KIRO_CONFIG_DIR/mcp.json"
    
    # Create MCP configuration
    cat > /tmp/mysql-mcp-config.json << EOF
{
  "mcpServers": {
    "mysql-server": {
      "url": "http://localhost:8080/mcp",
      "disabled": false,
      "autoApprove": ["test_connection"]
    }
  }
}
EOF

    echo "📋 Kiro MCP configuration:"
    cat /tmp/mysql-mcp-config.json
    echo ""
    
    read -p "🤔 Would you like to add this to your Kiro MCP configuration? (y/n): " -n 1 -r
    echo
    
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        # Backup existing config if it exists
        if [ -f "$MCP_CONFIG_FILE" ]; then
            cp "$MCP_CONFIG_FILE" "$MCP_CONFIG_FILE.backup.$(date +%s)"
            echo "💾 Backed up existing MCP config"
        fi
        
        # Merge or create new config
        if [ -f "$MCP_CONFIG_FILE" ]; then
            # TODO: Proper JSON merging would be better, but this is a simple approach
            echo "⚠️  Please manually merge the configuration above into $MCP_CONFIG_FILE"
            echo "   A backup was created at $MCP_CONFIG_FILE.backup.*"
        else
            mkdir -p "$KIRO_CONFIG_DIR"
            cp /tmp/mysql-mcp-config.json "$MCP_CONFIG_FILE"
            echo "✅ Created Kiro MCP configuration at $MCP_CONFIG_FILE"
        fi
    fi
    
    rm /tmp/mysql-mcp-config.json
fi

# Check for Claude Desktop
CLAUDE_CONFIG_FILE=""
if [[ "$OSTYPE" == "darwin"* ]]; then
    CLAUDE_CONFIG_FILE="$HOME/Library/Application Support/Claude/claude_desktop_config.json"
elif [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
    CLAUDE_CONFIG_FILE="$APPDATA/Claude/claude_desktop_config.json"
fi

if [ -n "$CLAUDE_CONFIG_FILE" ]; then
    echo ""
    echo "🤖 Claude Desktop configuration:"
    echo "⚠️  Note: Claude Desktop may not support HTTP MCP servers yet."
    echo "If supported, add this to $CLAUDE_CONFIG_FILE:"
    echo ""
    cat << EOF
{
  "mcpServers": {
    "mysql-server": {
      "url": "http://localhost:8080/mcp"
    }
  }
}
EOF
fi

echo ""
echo "🎉 Setup Complete!"
echo "=================="
echo ""
echo "⚠️  IMPORTANT: This is a Streamable HTTP MCP Server"
echo "   You need to START the server before using it with MCP clients!"
echo ""
echo "Next steps:"
echo "1. 📝 Edit config.toml with your MySQL database credentials"
echo "2. 🚀 Start the server: cargo run --release"
echo "3. 🔗 The server will be available at http://localhost:8080/mcp"
echo "4. 🔄 Restart your MCP client to load the new configuration"
echo "5. ✅ Test the connection using the 'test_connection' tool"
echo ""
echo "📚 For more configuration options, see:"
echo "   - README.md"
echo "   - mcp-config-examples.md"
echo ""
echo "🐛 If you encounter issues:"
echo "   - Check the logs with: RUST_LOG=debug cargo run"
echo "   - Verify your database is accessible"
echo "   - Ensure config.toml has correct credentials"
echo "   - Test HTTP endpoint: curl http://localhost:8080/mcp"