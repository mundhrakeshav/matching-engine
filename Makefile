.DEFAULT_GOAL := help

.PHONY: help build run run-local test fmt lint check clean

help:
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*##/ {printf "%-12s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## Build the optimized release binary.
	cargo build --release

run: ## Run with the current environment.
	cargo run

run-local: ## Load .env and run locally.
	ENV=local cargo run

test: ## Run all tests.
	cargo test

fmt: ## Verify Rust formatting.
	cargo fmt --check

lint: ## Run strict Clippy checks.
	cargo clippy --all-targets --all-features -- -D warnings

check: fmt lint test ## Run all local quality checks.

clean: ## Remove Cargo build artifacts.
	cargo clean
