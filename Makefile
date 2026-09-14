.PHONY: fmt lint lint-hotplug lint-tpm2 test test-hotplug test-tpm2 build ui ui-test static package check qualify emulator-vcan emulator-swtpm emulator-v4l2 emulator

fmt:
	cargo fmt --all -- --check

# Default features only, matching CI's rust-native job: the optional `tpm2`
# feature needs libtss2-dev, which most contributors won't have installed -
# see lint-tpm2 below for that leg specifically.
lint:
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

test:
	cargo test --all

test-hotplug:
	cargo test --all --features hotplug

test-tpm2:
	cargo test --all --features tpm2

build:
	cargo build --release

ui:
	cd web/dashboard && npm install --no-audit --no-fund && npm run build

ui-test:
	cd web/dashboard && npm install --no-audit --no-fund && npm run test

static:
	python3 scripts/check-static.py

qualify:
	python3 scripts/qualify-matrix.py

emulator-vcan:
	DA_EMULATOR_STRICT=$${DA_EMULATOR_STRICT:-1} ./scripts/emulator/smoke-vcan.sh

emulator-swtpm:
	DA_EMULATOR_STRICT=$${DA_EMULATOR_STRICT:-1} ./scripts/emulator/smoke-swtpm.sh

emulator-v4l2:
	DA_EMULATOR_STRICT=$${DA_EMULATOR_STRICT:-0} ./scripts/emulator/smoke-v4l2.sh

emulator:
	./scripts/emulator/smoke-all.sh

check: static lint test ui-test ui

package: check
	./scripts/package.sh
