# Use this file purely for shortcuts only

all: test fmt lint

.PHONY: fmt
fmt:
	cargo fmt --

.PHONY: lint
lint:
	cargo clippy -- -D warnings

.PHONY: test
test:
	cargo test
	cargo test -- --ignored --test-threads=1
