# Use this file purely for shortcuts only

all: test fmt lint

fmt:
	cargo +nightly fmt -- --write-mode=diff

lint:
	cargo +nightly clippy -- -D warnings

test:
	cargo test
	cargo test -- --ignored --test-threads=1
