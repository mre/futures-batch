# Needed SHELL since I'm using zsh
SHELL := /bin/bash

.PHONY: help
help: ## This help message
	@echo -e "$$(grep -hE '^\S+:.*##' $(MAKEFILE_LIST) | sed -e 's/:.*##\s*/:/' -e 's/^\(.\+\):\(.*\)/\\x1b[36m\1\\x1b[m:\2/' | column -c2 -t -s :)"

.PHONY: build
build: ## Build Rust code locally
	cargo build

.PHONY: docs
docs: ## Generate and show documentation
	cargo doc --open 

.PHONY: test
test: ## Run tests
	cargo test

.PHONY: lint
lint: ## Run linter
	cargo clippy --all-targets --all-features -- -D warnings

.PHONY: publish
publish: ## Publish to crates.io
	cargo publish