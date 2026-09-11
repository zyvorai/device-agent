.PHONY: fmt lint test build ui ui-test static package check

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets --all-features

test:
	cargo test --all

build:
	cargo build --release

ui:
	cd web/dashboard && npm install --no-audit --no-fund && npm run build

ui-test:
	cd web/dashboard && npm install --no-audit --no-fund && npm run test

static:
	python3 scripts/check-static.py

check: static lint test ui-test ui

package: check
	./scripts/package.sh
