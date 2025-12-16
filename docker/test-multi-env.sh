#!/bin/bash
# Comprehensive Multi-Environment Testing Script for MySQL MCP Server

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Test configuration
TEST_ENVIRONMENT=""
VERBOSE=false
QUICK_TEST=false

echo -e "${GREEN}🧪 MySQL MCP Server Multi-Environment Test Suite${NC}"
echo "=================================================="

# Function to show usage
show_usage() {
    echo "Usage: $0 [environment] [options]"
    echo ""
    echo "Environments:"
    echo "  dev         Test development environment"
    echo "  staging     Test staging environment"
    echo "  multi       Test multi-environment setup (default)"
    echo "  all         Test all environments sequentially"
    echo ""
    echo "Options:"
    echo "  --verbose, -v       Show detailed output"
    echo "  --quick, -q         Run quick tests only"
    echo "  --help, -h          Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 multi --verbose              # Test multi-env with detailed output"
    echo "  $0 dev --quick                  # Quick test of dev environment"
    echo "  $0 all                          # Test all environments"
}

# Function to run a test with status reporting
run_test() {
    local test_name="$1"
    local test_command="$2"
    local expected_pattern="$3"
    
    if [ "$VERBOSE" = true ]; then
        echo -e "${BLUE}🔍 Running: $test_name${NC}"
    fi
    
    echo -n "  $test_name... "
    
    if [ "$VERBOSE" = true ]; then
        echo ""
        echo -e "${CYAN}Command: $test_command${NC}"
    fi
    
    local result
    result=$(eval "$test_command" 2>&1)
    local exit_code=$?
    
    if [ "$VERBOSE" = true ]; then
        echo -e "${CYAN}Response: $result${NC}"
    fi
    
    if [ $exit_code -eq 0 ] && echo "$result" | grep -q "$expected_pattern"; then
        echo -e "${GREEN}✅${NC}"
        return 0
    else
        echo -e "${RED}❌${NC}"
        if [ "$VERBOSE" = true ]; then
            echo -e "${RED}Expected pattern: $expected_pattern${NC}"
            echo -e "${RED}Actual result: $result${NC}"
        fi
        return 1
    fi
}

# Function to test basic MCP functionality
test_basic_mcp() {
    echo -e "${YELLOW}📋 Basic MCP Protocol Tests${NC}"
    
    local failed=0
    
    # Test 1: Health Check
    run_test "Health Check" \
        "curl -s -f http://localhost:8080/health" \
        '"status":"healthy"' || ((failed++))
    
    # Test 2: MCP Initialize
    run_test "MCP Initialize" \
        "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}'" \
        '"protocolVersion"' || ((failed++))
    
    # Test 3: List Tools
    run_test "List Tools" \
        "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}'" \
        '"execute_query"' || ((failed++))
    
    if [ $failed -eq 0 ]; then
        echo -e "${GREEN}✅ All basic MCP tests passed${NC}"
    else
        echo -e "${RED}❌ $failed basic MCP tests failed${NC}"
    fi
    
    return $failed
}

# Function to test multi-environment functionality
test_multi_environment() {
    echo -e "${YELLOW}🌐 Multi-Environment Tests${NC}"
    
    local failed=0
    
    # Test 1: List Environments
    run_test "List Environments" \
        "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"list_environments\",\"arguments\":{}}}'" \
        '"result"' || ((failed++))
    
    # Test 2: Test Connection (Default Environment)
    run_test "Test Connection (Default)" \
        "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"test_connection\",\"arguments\":{}}}'" \
        '"status":"success"' || ((failed++))
    
    # Test 3: Execute Query on Specific Environment (Dev)
    run_test "Execute Query (Dev Environment)" \
        "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"tools/call\",\"params\":{\"name\":\"execute_query_env\",\"arguments\":{\"sql\":\"SELECT 1 as test_value\",\"environment\":\"dev\"}}}'" \
        '"test_value"' || ((failed++))
    
    if [ "$QUICK_TEST" = false ]; then
        # Test 4: Execute Query on Staging Environment
        run_test "Execute Query (Staging Environment)" \
            "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"tools/call\",\"params\":{\"name\":\"execute_query_env\",\"arguments\":{\"sql\":\"SELECT 2 as test_value\",\"environment\":\"staging\"}}}'" \
            '"test_value"' || ((failed++))
        
        # Test 5: Multi-Environment Query
        run_test "Multi-Environment Query" \
            "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/call\",\"params\":{\"name\":\"execute_query_multi_env\",\"arguments\":{\"sql\":\"SELECT COUNT(*) as table_count FROM information_schema.tables WHERE table_schema = DATABASE()\",\"environments\":[\"dev\",\"staging\"]}}}'" \
            '"results"' || ((failed++))
        
        # Test 6: List Databases with Environment Filter
        run_test "List Databases (Environment Filtered)" \
            "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":8,\"method\":\"tools/call\",\"params\":{\"name\":\"list_databases_env\",\"arguments\":{\"environment\":\"dev\"}}}'" \
            '"result"' || ((failed++))
    fi
    
    if [ $failed -eq 0 ]; then
        echo -e "${GREEN}✅ All multi-environment tests passed${NC}"
    else
        echo -e "${RED}❌ $failed multi-environment tests failed${NC}"
    fi
    
    return $failed
}

# Function to test database connectivity
test_database_connectivity() {
    echo -e "${YELLOW}🗄️  Database Connectivity Tests${NC}"
    
    local failed=0
    
    # Test direct MySQL connections
    if command -v mysql &> /dev/null; then
        # Test Dev Database
        run_test "Dev Database Direct Connection" \
            "mysql -h localhost -P 3306 -u dev_user -pdev_password -e 'SELECT 1' 2>/dev/null" \
            "1" || ((failed++))
        
        if [ "$QUICK_TEST" = false ]; then
            # Test Staging Database
            run_test "Staging Database Direct Connection" \
                "mysql -h localhost -P 3307 -u staging_user -pstaging_password -e 'SELECT 1' 2>/dev/null" \
                "1" || ((failed++))
        fi
    else
        echo -e "${YELLOW}⚠️  MySQL client not available, skipping direct database tests${NC}"
    fi
    
    # Test via MCP tools
    run_test "Database List via MCP" \
        "curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"tools/call\",\"params\":{\"name\":\"list_databases\",\"arguments\":{}}}'" \
        '"result"' || ((failed++))
    
    if [ $failed -eq 0 ]; then
        echo -e "${GREEN}✅ All database connectivity tests passed${NC}"
    else
        echo -e "${RED}❌ $failed database connectivity tests failed${NC}"
    fi
    
    return $failed
}

# Function to test performance and resource usage
test_performance() {
    if [ "$QUICK_TEST" = true ]; then
        return 0
    fi
    
    echo -e "${YELLOW}⚡ Performance Tests${NC}"
    
    local failed=0
    
    # Test response time
    echo -n "  Response Time Test... "
    local start_time=$(date +%s%N)
    curl -s -f http://localhost:8080/health > /dev/null
    local end_time=$(date +%s%N)
    local response_time=$(( (end_time - start_time) / 1000000 ))  # Convert to milliseconds
    
    if [ $response_time -lt 1000 ]; then  # Less than 1 second
        echo -e "${GREEN}✅ (${response_time}ms)${NC}"
    else
        echo -e "${YELLOW}⚠️  (${response_time}ms - slow)${NC}"
        ((failed++))
    fi
    
    # Test concurrent requests
    echo -n "  Concurrent Requests Test... "
    local concurrent_failed=0
    for i in {1..5}; do
        curl -s -f http://localhost:8080/health > /dev/null &
    done
    wait
    
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✅${NC}"
    else
        echo -e "${RED}❌${NC}"
        ((failed++))
    fi
    
    # Check resource usage
    echo -e "${BLUE}📊 Resource Usage:${NC}"
    docker stats --no-stream --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}" | grep mysql-mcp || echo "  No containers found"
    
    if [ $failed -eq 0 ]; then
        echo -e "${GREEN}✅ Performance tests completed${NC}"
    else
        echo -e "${YELLOW}⚠️  Some performance issues detected${NC}"
    fi
    
    return $failed
}

# Function to test specific environment
test_environment() {
    local env=$1
    echo -e "${BLUE}🧪 Testing $env environment${NC}"
    echo "================================"
    
    # Check if environment is running
    local compose_file
    case $env in
        dev)
            compose_file="docker-compose.dev-only.yml"
            ;;
        staging)
            compose_file="docker-compose.staging.yml"
            ;;
        multi)
            compose_file="docker-compose.multi-env.yml"
            ;;
        *)
            echo -e "${RED}❌ Unknown environment: $env${NC}"
            return 1
            ;;
    esac
    
    if ! docker-compose -f "$compose_file" ps | grep -q "Up"; then
        echo -e "${RED}❌ $env environment is not running${NC}"
        echo "Please start it first with: ./manage-environments.sh start $env"
        return 1
    fi
    
    local total_failed=0
    
    # Run test suites
    test_basic_mcp || ((total_failed++))
    echo ""
    
    if [ "$env" = "multi" ]; then
        test_multi_environment || ((total_failed++))
        echo ""
    fi
    
    test_database_connectivity || ((total_failed++))
    echo ""
    
    test_performance || ((total_failed++))
    echo ""
    
    # Summary
    if [ $total_failed -eq 0 ]; then
        echo -e "${GREEN}🎉 All tests passed for $env environment!${NC}"
    else
        echo -e "${RED}❌ $total_failed test suite(s) failed for $env environment${NC}"
    fi
    
    return $total_failed
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        dev|staging|multi|all)
            TEST_ENVIRONMENT=$1
            shift
            ;;
        --verbose|-v)
            VERBOSE=true
            shift
            ;;
        --quick|-q)
            QUICK_TEST=true
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

# Default to multi environment if none specified
if [ -z "$TEST_ENVIRONMENT" ]; then
    TEST_ENVIRONMENT="multi"
fi

# Change to docker directory
cd "$(dirname "$0")"

echo -e "${BLUE}Test Configuration:${NC}"
echo "  Environment: $TEST_ENVIRONMENT"
echo "  Verbose: $VERBOSE"
echo "  Quick Test: $QUICK_TEST"
echo ""

# Run tests
case $TEST_ENVIRONMENT in
    all)
        echo -e "${BLUE}🔄 Testing all environments sequentially${NC}"
        total_failures=0
        
        for env in dev staging multi; do
            echo ""
            test_environment "$env" || ((total_failures++))
        done
        
        echo ""
        echo "================================"
        if [ $total_failures -eq 0 ]; then
            echo -e "${GREEN}🎉 All environments passed testing!${NC}"
        else
            echo -e "${RED}❌ $total_failures environment(s) failed testing${NC}"
        fi
        exit $total_failures
        ;;
    *)
        test_environment "$TEST_ENVIRONMENT"
        exit $?
        ;;
esac