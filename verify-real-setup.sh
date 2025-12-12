#!/bin/bash

# Verification script for real environment setup

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔍 Verifying Real Environment Setup${NC}"
echo ""

# Check 1: Verify docker-compose.real.yml exists and is gitignored
echo -e "${YELLOW}1. Checking docker-compose.real.yml...${NC}"
if [ -f "docker-compose.real.yml" ]; then
    echo -e "${GREEN}   ✅ docker-compose.real.yml exists${NC}"
    
    # Check if it's gitignored
    if git check-ignore docker-compose.real.yml > /dev/null 2>&1; then
        echo -e "${GREEN}   ✅ docker-compose.real.yml is properly gitignored${NC}"
    else
        echo -e "${RED}   ❌ docker-compose.real.yml is NOT gitignored!${NC}"
        exit 1
    fi
    
    # Validate syntax
    if docker-compose -f docker-compose.real.yml config --quiet > /dev/null 2>&1; then
        echo -e "${GREEN}   ✅ docker-compose.real.yml syntax is valid${NC}"
    else
        echo -e "${RED}   ❌ docker-compose.real.yml has syntax errors${NC}"
        exit 1
    fi
else
    echo -e "${RED}   ❌ docker-compose.real.yml not found${NC}"
    exit 1
fi

# Check 2: Verify scripts exist and are executable
echo -e "${YELLOW}2. Checking real environment scripts...${NC}"
scripts=("docker/start-real.sh" "docker/test-real.sh" "docker/cleanup-real.sh")
for script in "${scripts[@]}"; do
    if [ -f "$script" ] && [ -x "$script" ]; then
        echo -e "${GREEN}   ✅ $script exists and is executable${NC}"
    else
        echo -e "${RED}   ❌ $script missing or not executable${NC}"
        exit 1
    fi
done

# Check 3: Verify Makefile has real environment targets
echo -e "${YELLOW}3. Checking Makefile targets...${NC}"
targets=("real-start" "real-test" "real-clean" "real-logs")
for target in "${targets[@]}"; do
    if grep -q "^$target:" Makefile; then
        echo -e "${GREEN}   ✅ Makefile target '$target' exists${NC}"
    else
        echo -e "${RED}   ❌ Makefile target '$target' missing${NC}"
        exit 1
    fi
done

# Check 4: Verify config.toml is gitignored
echo -e "${YELLOW}4. Checking config.toml...${NC}"
if [ -f "config.toml" ]; then
    if git check-ignore config.toml > /dev/null 2>&1; then
        echo -e "${GREEN}   ✅ config.toml is properly gitignored${NC}"
    else
        echo -e "${RED}   ❌ config.toml is NOT gitignored!${NC}"
        exit 1
    fi
else
    echo -e "${YELLOW}   ⚠️  config.toml not found (this is OK if using env vars)${NC}"
fi

# Check 5: Verify documentation exists
echo -e "${YELLOW}5. Checking documentation...${NC}"
docs=("docker/README-REAL.md" "REAL-ENVIRONMENT.md")
for doc in "${docs[@]}"; do
    if [ -f "$doc" ]; then
        echo -e "${GREEN}   ✅ $doc exists${NC}"
    else
        echo -e "${RED}   ❌ $doc missing${NC}"
        exit 1
    fi
done

# Check 6: Verify .gitignore entries
echo -e "${YELLOW}6. Checking .gitignore entries...${NC}"
gitignore_entries=("docker-compose.real.yml" "docker-compose.production.yml" "config.toml")
for entry in "${gitignore_entries[@]}"; do
    if grep -q "$entry" .gitignore; then
        echo -e "${GREEN}   ✅ '$entry' is in .gitignore${NC}"
    else
        echo -e "${RED}   ❌ '$entry' missing from .gitignore${NC}"
        exit 1
    fi
done

# Summary
echo ""
echo -e "${GREEN}🎉 All checks passed! Real environment setup is correct.${NC}"
echo ""
echo -e "${BLUE}📋 Quick Start Commands:${NC}"
echo -e "${BLUE}   Start real environment: make real-start${NC}"
echo -e "${BLUE}   Test real environment:  make real-test${NC}"
echo -e "${BLUE}   View logs:             make real-logs${NC}"
echo -e "${BLUE}   Clean up:              make real-clean${NC}"
echo ""
echo -e "${YELLOW}⚠️  Remember: The real environment connects to AWS RDS Aurora!${NC}"