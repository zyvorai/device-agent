.PHONY: fmt lint lint-hotplug lint-tpm2 test test-hotplug test-tpm2 build ui ui-test static package check check-ui qualify hil emulator-vcan emulator-swtpm emulator-v4l2 emulator package-deb-rpm help ci status deploy-remote

fmt: ## cargo fmt --check
	cargo fmt --all -- --check

# Default features only, matching CI's rust-native job: the optional `tpm2`
# feature needs libtss2-dev, which most contributors won't have installed -
# see lint-tpm2 below for that leg specifically.
lint: ## Clippy, default features, warnings denied
	cargo clippy --all-targets -- -D warnings

# Not part of the default `check` target: netlink-sys (AF_NETLINK,
# sockaddr_nl) is Linux-only at compile time, unlike tpm2's Rust code, which
# compiles anywhere and only needs a system library at build time. Run this
# on Linux, or let CI's rust-native job cover it.
lint-hotplug:
	cargo clippy --all-targets --features hotplug -- -D warnings

# Needs libtss2-dev (apt: libtss2-dev, dnf: tpm2-tss-devel) - not part of the
# default `check` target for that reason, matching CI's separate rust-tpm2 job.
lint-tpm2:
	cargo clippy --all-targets --features tpm2 -- -D warnings

test: ## All tests, default features
	cargo test --all

test-hotplug:
	cargo test --all --features hotplug

test-tpm2:
	cargo test --all --features tpm2

build: ## Release binaries, including agentctl
	cargo build --release

ui:
	cd web/dashboard && npm install --no-audit --no-fund && npm run build

ui-test:
	cd web/dashboard && npm install --no-audit --no-fund && npm run test

static:
	python3 scripts/check-static.py

qualify:
	python3 scripts/qualify-matrix.py

hil:
	DA_HIL_STRICT=$${DA_HIL_STRICT:-1} ./scripts/hil/run-minewing-hil.sh

emulator-vcan:
	DA_EMULATOR_STRICT=$${DA_EMULATOR_STRICT:-1} ./scripts/emulator/smoke-vcan.sh

emulator-swtpm:
	DA_EMULATOR_STRICT=$${DA_EMULATOR_STRICT:-1} ./scripts/emulator/smoke-swtpm.sh

emulator-v4l2:
	DA_EMULATOR_STRICT=$${DA_EMULATOR_STRICT:-0} ./scripts/emulator/smoke-v4l2.sh

emulator:
	./scripts/emulator/smoke-all.sh

# rust-native gate. Dashboard npm is separate: @types/react 19 and
# @types/react-dom 18 do not resolve, and that job is already red in CI.
check: static lint test build ## fmt, clippy, tests, release build

check-ui: ui-test ui ## Dashboard install, test, and build

package: check ui
	./scripts/package.sh

ci: check ## Same core gate as the rust-native CI job

status: build ## agentctl status from the example config (or CONFIG=path)
	./target/release/agentctl --config $(or $(CONFIG),config/device-agent.example.toml) status

deploy-remote: ## Deploy: make deploy-remote H=<host> [U=sus] [ARGS=--quick]
	@test -n "$(H)" || (echo "Usage: make deploy-remote H=<host> [U=user] [ARGS='--quick --no-ui']"; exit 1)
	./scripts/deploy-remote.sh $(if $(U),$(U)@)$(H) $(ARGS)

help: ## Show targets
	@grep -E '^[a-zA-Z0-9_-]+:.*## ' $(MAKEFILE_LIST) | sort | awk -F':.*## ' '{printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'

package-deb-rpm:
	./scripts/package-deb-rpm.sh
