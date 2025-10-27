# Use this file purely for shortcuts only
REDIS_VERSION=8.2.2
VALKEY_VERSION=9.0.0
CELL_VERSION=0.4.0

REDIS_CELL_IMAGE_NAME=redis-cell
REDIS_CELL_IMAGE_TAG=${REDIS_VERSION}-${CELL_VERSION}

VALKEY_CELL_IMAGE_NAME=valkey-cell
VALKEY_CELL_IMAGE_TAG=${VALKEY_VERSION}-${CELL_VERSION}

all: test fmt lint

# Documented commands will appear in the help text.
#
# Derived from: https://github.com/contribsys/faktory/blob/4e7b8196a14c60f222be1f63bdcced2c1a750971/Makefile#L252-L253
.PHONY: help
help:
	@grep -E '^[/0-9a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

.PHONY: fmt
fmt: ## Run formatter
	cargo fmt --

.PHONY: lint
lint: ## Run clippy checks
	cargo clippy -- -D warnings

.PHONY: test
test: ## Run unit tests
	cargo test
	cargo test -- --ignored --test-threads=1

.PHONY: test/e2e
test/e2e: test/e2e/redis test/e2e/valkey ## Run end-to-end tests

.PHONY: test/e2e/redis
test/e2e/redis: ## Run end-to-end tests against Redis
	cargo test --features e2e-test --test e2e -- --nocapture

.PHONY: test/e2e/valkey
test/e2e/valkey: ## Run end-to-end tests against Valkey
	cargo test --features e2e-test,valkey --test e2e -- --nocapture

.PHONY: images/redis
images/redis: ## Build Redis with Redis Cell module docker image
	docker build . -f redis.Dockerfile -t ${REDIS_CELL_IMAGE_NAME}:${REDIS_CELL_IMAGE_TAG}

.PHONY: images/valkey
images/valkey: ## Build Valkey with Redis Cell module docker image
	docker build . -f valkey.Dockerfile -t ${VALKEY_CELL_IMAGE_NAME}:${VALKEY_CELL_IMAGE_TAG}

.PHONY: images
images: images/redis images/valkey ## Build images for both Redis and Valkey with Redis Cell module

