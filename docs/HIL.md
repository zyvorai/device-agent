---
hero:
  eyebrow: HIL
  title: Minewing GW1 r1 hardware-in-the-loop
---

Physical Minewing HIL is **operator-signed**. Emulator CI and lab-surrogate
runs do **not** close Minewing claimable rows.

## GitHub CI (lab substitute)

When Minewing silicon is unavailable, CI job **`hil-ci-emulator`** runs
[`scripts/ci/run-hil-emulator.sh`](https://github.com/zyvorai/device-agent/blob/main/scripts/ci/run-hil-emulator.sh):
starts the agent with `minewing-gw1-r1` profile + vcan, then
`DA_HIL_ENV=ci-emulator` against the HIL harness. Evidence under
`evidence/qualification/ci/` — **`minewing_claimable` is always false**.

## Runner (physical — still open)

```bash
# On the Minewing board (or SSH tunnel to its API):
DA_HIL_ENV=physical \
DA_HIL_BASE=http://127.0.0.1:9188 \
DA_HIL_PROFILE=minewing-gw1-r1 \
./scripts/hil/run-minewing-hil.sh

# Optional: attach WAN-loss evidence, then sign only if claimable:
DA_HIL_WAN_LOSS_LOG=/path/to/wan-loss.log \
DA_HIL_SIGN=1 DA_HIL_ENV=physical ./scripts/hil/run-minewing-hil.sh
```

Evidence lands in `evidence/qualification/hil/<stamp>/` (`results.json`,
`SUMMARY.md`, raw JSON captures). `DA_HIL_SIGN=1` updates
`hardware-checklist.md` **only** when `minewing_claimable=true`
(physical env, profile match, zero fail/blocked).

## Lab surrogate (recorded; not claimable)

An x86 lab host with Device Agent may run:

```bash
DA_HIL_ENV=lab-surrogate DA_HIL_STRICT=0 ./scripts/hil/run-minewing-hil.sh
```

Recorded stamp example:
[`20260914T162450Z`](https://github.com/zyvorai/device-agent/blob/main/evidence/qualification/hil/20260914T162450Z/SUMMARY.md)
(`environment=lab-surrogate`, `minewing_claimable=false`). That captures
wiring evidence; it **must not** be signed as Minewing silicon.
