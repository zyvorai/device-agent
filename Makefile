.PHONY: fmt lint test build ui ui-test package check

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test --all

build:
	cargo build --release

ui:
	cd web/dashboard && npm install --no-audit --no-fund && npm run build

ui-test:
	cd web/dashboard && npm install --no-audit --no-fund && npm run test

check: fmt lint test ui-test ui

package: check
	./scripts/package.sh
