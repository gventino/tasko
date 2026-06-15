# tasko-fable — developer tasks
# Run `make` or `make help` to list available targets.

CARGO ?= cargo
BIN    := tasko-fable

# Number of demo tasks for `make seed` (override: make seed N=500)
N ?= 100

# Optional SQLite file (override: make run DB=/tmp/demo.db)
DB ?=

ifneq ($(strip $(DB)),)
export TASKO_DB := $(DB)
endif

.DEFAULT_GOAL := help

.PHONY: help build run seed test lint fmt fmt-check check ci install uninstall clean

help: ## Show this help
	@awk 'BEGIN {FS = ":.*##"; printf "tasko-fable — make targets\n\nUsage: make \033[36m<target>\033[0m\n\n"} /^[a-zA-Z0-9_-]+:.*##/ {printf "  \033[36m%-11s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## Build the optimized release binary (target/release/)
	$(CARGO) build --release --locked

run: ## Run the app in release mode (override with DB=/path/to.db)
	$(CARGO) run --release

seed: ## Seed N demo tasks into the database (default N=100)
	$(CARGO) run --release -- --seed $(N)

test: ## Run the full test suite
	$(CARGO) test

lint: ## Lint all targets with clippy
	$(CARGO) clippy --all-targets

fmt: ## Format the codebase with rustfmt
	$(CARGO) fmt

fmt-check: ## Check formatting without modifying files
	$(CARGO) fmt --check

check: ## Type-check the project without producing binaries
	$(CARGO) check --all-targets

ci: fmt-check lint test ## Run formatting check, lint and tests (what CI runs)

install: ## Build and install tasko-fable into ~/.cargo/bin
	$(CARGO) install --path . --locked --force

uninstall: ## Remove the installed tasko-fable binary
	$(CARGO) uninstall $(BIN)

clean: ## Remove build artifacts
	$(CARGO) clean
