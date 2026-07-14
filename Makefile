.PHONY: check test clippy fmt build dev clean help ax-helper

MANIFEST := src-tauri/Cargo.toml

help:
	@echo "Ghost development tasks:"
	@echo "  make check   - Run cargo check"
	@echo "  make test    - Run all tests"
	@echo "  make clippy  - Run clippy linter"
	@echo "  make fmt     - Format code"
	@echo "  make fmt-check - Check format without fixing"
	@echo "  make build   - Build desktop app (no bundle)"
	@echo "  make dev     - Run dev app"
	@echo "  make ax-helper - Build GhostAXHelper sidecar (macOS only)"
	@echo "  make clean   - Clean build artifacts"
	@echo "  make ci      - Run all CI checks (fmt-check, clippy, test)"

check:
	cargo check --manifest-path $(MANIFEST) --all-targets

test:
	cargo test --manifest-path $(MANIFEST)

clippy:
	cargo clippy --manifest-path $(MANIFEST) --all-targets -- -D warnings

fmt:
	cargo fmt --manifest-path $(MANIFEST)

fmt-check:
	cargo fmt --manifest-path $(MANIFEST) -- --check

build:
	cargo tauri build --no-bundle

dev:
	cargo tauri dev

ax-helper:
	bash scripts/build-ghost-ax-helper.sh

icons:
	bash scripts/generate-brand-icons.sh

clean:
	cargo clean

ci: fmt-check clippy test
	@echo "✅ All CI checks passed"
