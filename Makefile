# Makefile for MySQL MCP Server

.PHONY: help build test clean docker-build docker-start docker-test docker-clean docker-dev real-start real-test real-clean real-logs

# Default target
help:
	@echo "MySQL MCP Server - Available commands:"
	@echo ""
	@echo "Local Development:"
	@echo "  build          Build the project"
	@echo "  test           Run tests"
	@echo "  run            Run the server locally"
	@echo "  clean          Clean build artifacts"
	@echo ""
	@echo "Docker Commands:"
	@echo "  docker-build   Build Docker images"
	@echo "  docker-start   Start services in production mode"
	@echo "  docker-dev     Start services in development mode"
	@echo "  docker-test    Test Docker deployment"
	@echo "  docker-clean   Clean up Docker resources"
	@echo "  docker-logs    View Docker logs"
	@echo ""
	@echo "Real Environment (AWS RDS Aurora):"
	@echo "  real-start     Start with real AWS RDS database"
	@echo "  real-test      Test real environment deployment"
	@echo "  real-clean     Clean up real environment"
	@echo "  real-logs      View real environment logs"
	@echo ""
	@echo "Utilities:"
	@echo "  verify         Verify setup and security"
	@echo "  format         Format code"
	@echo "  lint           Run clippy lints"

# Local development
build:
	cargo build --release

test:
	cargo test

run:
	cargo run --release

clean:
	cargo clean

# Docker commands
docker-build:
	./docker/build.sh

docker-start:
	./docker/start.sh --prod

docker-dev:
	./docker/start.sh --dev

docker-test:
	./docker/test.sh

docker-clean:
	./docker/cleanup.sh

docker-clean-all:
	./docker/cleanup.sh --full

docker-logs:
	docker-compose logs -f

docker-logs-dev:
	docker-compose -f docker-compose.dev.yml logs -f

# Real environment commands (AWS RDS Aurora)
real-start:
	./docker/start-real.sh

real-test:
	./docker/test-real.sh

real-clean:
	./docker/cleanup-real.sh

real-clean-all:
	./docker/cleanup-real.sh --full

real-logs:
	docker-compose -f docker-compose.real.yml logs -f

# Utilities
verify:
	./verify-setup.sh

format:
	cargo fmt

lint:
	cargo clippy -- -D warnings

# CI/CD targets
ci-test: build test docker-build docker-start docker-test docker-clean

# Development workflow
dev-setup: verify docker-build docker-dev
	@echo "Development environment ready!"
	@echo "Access the server at: http://localhost:8080"
	@echo "View logs with: make docker-logs-dev"

# Production deployment
prod-deploy: build test docker-build docker-start docker-test
	@echo "Production deployment complete!"
	@echo "Access the server at: http://localhost:8080"
	@echo "View logs with: make docker-logs"