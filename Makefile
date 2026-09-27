CARGO ?= cargo
# `wasm32v1-none`, not `wasm32-unknown-unknown`. soroban-sdk 28's build script
# rejects the older target on Rust 1.82+ because that target enables
# `reference-types` and `multi-value`, which the Soroban environment does not
# support. `stellar contract build` uses this target too.
WASM_TARGET ?= wasm32v1-none
WASM_BUILD_FLAGS ?= --target $(WASM_TARGET) --release --no-default-features
# soroban-sdk >= 28 refuses to build unless the build system is told it
# supports spec shaking v2. `stellar contract build` sets this itself; these
# targets call cargo directly, so they set it here.
export SOROBAN_SDK_BUILD_SYSTEM_SUPPORTS_SPEC_SHAKING_V2 := 1

.PHONY: all preflight build test fmt fmt-check lint clippy wasm clean install-hooks

all: fmt-check lint test

## Verify the local toolchain can build this crate.
preflight:
	@$(CARGO) --version
	@rustc --version
	@rustup target list --installed | grep -q '$(WASM_TARGET)' \
		&& echo "wasm target: $(WASM_TARGET) installed" \
		|| (echo "missing wasm target: run 'rustup target add $(WASM_TARGET)'" && exit 1)

## Build the contract for the host target.
build:
	$(CARGO) build

## Run the unit test suite.
test:
	$(CARGO) test

## Format the whole workspace.
fmt:
	$(CARGO) fmt --all

## Check formatting without rewriting files.
fmt-check:
	$(CARGO) fmt --all --check

## Lint with warnings denied.
#
# Clippy is split in two on purpose. The crate is `#![no_std]` and defines a
## `#[panic_handler]` for the wasm target only, so the host library target
## cannot be built at all and `--all-targets` fails on the host. The contract
## is linted for the target it ships as; the tests are linted on the host.
## `--all-features` is omitted for the wasm target because soroban-sdk's
## `testutils` feature is not supported there.
lint clippy:
	$(CARGO) clippy --lib --target $(WASM_TARGET) -- -D warnings
	$(CARGO) clippy --tests --all-features -- -D warnings

## Build the optimized wasm artifact for on-chain deployment.
wasm:
	$(CARGO) build $(WASM_BUILD_FLAGS)

clean:
	$(CARGO) clean

## Install the pre-commit hook that runs fmt and clippy.
install-hooks:
	@mkdir -p .git/hooks
	@cp .hooks/pre-commit .git/hooks/pre-commit
	@chmod +x .git/hooks/pre-commit
	@echo "installed .git/hooks/pre-commit"
