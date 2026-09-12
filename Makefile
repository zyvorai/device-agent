.PHONY: fmt lint lint-hotplug lint-tpm2 test test-hotplug test-tpm2 build ui ui-test static package check

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

check: static lint test ui-test ui

package: check
	./scripts/package.sh
