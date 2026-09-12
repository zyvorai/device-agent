# Contributing

1. Open an issue describing the hardware, driver or integration change.
2. Keep hardware discovery separate from protocol interpretation.
3. Add a fixture or unit test for every parser.
4. Run `make check` before submitting a PR. It covers default features only;
   if you touched `--features tpm2` or `--features hotplug` code, also run
   `make lint-tpm2 test-tpm2` (needs `libtss2-dev`) and/or
   `make lint-hotplug test-hotplug` (Linux only — `netlink-sys` doesn't
   compile on macOS) — CI runs both regardless.
5. Avoid introducing Kubernetes as a runtime dependency.

All contributions are accepted under Apache-2.0.
