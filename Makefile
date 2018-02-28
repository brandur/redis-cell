# Use this file purely for shortcuts only

all: test fmt

fmt:
	cargo +nightly fmt -- --write-mode=diff

test:
	cargo test
	cargo test -- --ignored --test-threads=1
