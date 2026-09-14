---
hero:
  eyebrow: HIL
  title: Minewing GW1 r1 hardware-in-the-loop
---

Physical / QEMU HIL is **operator-signed**. Emulator CI does not close these rows.

## Runner

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

## Lab surrogate

An x86 lab host with Device Agent may run:

```bash
DA_HIL_ENV=lab-surrogate DA_HIL_STRICT=0 ./scripts/hil/run-minewing-hil.sh
```

That captures wiring evidence; it **must not** be signed as Minewing silicon.
