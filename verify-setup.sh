#!/bin/bash

# Verify Git and Security Setup Script
# This script verifies that the repository is properly configured with security in mind

echo "🔍 Verifying Git and Security Setup..."
echo

# Check if we're in a git repository
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    echo "❌ Not in a Git repository"
    exit 1
fi

echo "✅ Git repository detected"

# Check remote origin
REMOTE_URL=$(git remote get-url origin 2>/dev/null)
if [[ $REMOTE_URL == *"streamable-db-mcp-server"* ]]; then
    echo "✅ Remote origin correctly set to: $REMOTE_URL"
else
    echo "⚠️  Remote origin: $REMOTE_URL"
fi

# Check if sensitive files are ignored
echo
echo "🔒 Checking sensitive file protection..."

SENSITIVE_FILES=("config.toml" "*.log" ".env")
for file in "${SENSITIVE_FILES[@]}"; do
    if git check-ignore "$file" > /dev/null 2>&1; then
        echo "✅ $file is properly ignored"
    else
        echo "⚠️  $file might not be ignored"
    fi
done

# Check if config.example.toml exists
if [[ -f "config.example.toml" ]]; then
    echo "✅ config.example.toml exists (template file)"
else
    echo "❌ config.example.toml missing"
fi

# Check if actual config.toml exists but is ignored
if [[ -f "config.toml" ]]; then
    if git check-ignore "config.toml" > /dev/null 2>&1; then
        echo "✅ config.toml exists but is properly ignored"
    else
        echo "❌ config.toml exists but is NOT ignored - SECURITY RISK!"
        exit 1
    fi
else
    echo "ℹ️  config.toml not found (will need to be created from example)"
fi

# Check git status
echo
echo "📊 Git Status:"
UNTRACKED=$(git status --porcelain | grep "^??")
if [[ -n "$UNTRACKED" ]]; then
    echo "⚠️  Untracked files found:"
    echo "$UNTRACKED"
    echo "   Make sure no sensitive files are untracked!"
else
    echo "✅ No untracked files"
fi

MODIFIED=$(git status --porcelain | grep "^.M")
if [[ -n "$MODIFIED" ]]; then
    echo "ℹ️  Modified files:"
    echo "$MODIFIED"
fi

# Check if we can build
echo
echo "🔨 Testing build..."
if cargo check --quiet; then
    echo "✅ Project builds successfully"
else
    echo "❌ Build failed"
    exit 1
fi

echo
echo "🎉 Setup verification complete!"
echo
echo "📋 Next steps:"
echo "1. Copy config.example.toml to config.toml"
echo "2. Edit config.toml with your database credentials"
echo "3. Run: cargo run"
echo "4. Test with: ./test-mcp-connection.sh"
echo
echo "🔒 Security reminders:"
echo "- Never commit config.toml (it contains sensitive data)"
echo "- Use config.example.toml as a template"
echo "- The .gitignore is configured to protect sensitive files"