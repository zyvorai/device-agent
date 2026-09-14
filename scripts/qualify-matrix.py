#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Software qualification matrix for Zyvor Device Agent."""
from __future__ import annotations

import json
import os
import pathlib
import subprocess
import sys
from datetime import datetime, timezone

ROOT = pathlib.Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "evidence" / "qualification"
VERSION = "0.1.5"
for line in (ROOT / "Cargo.toml").read_text().splitlines():
    if line.startswith("version"):
        VERSION = line.split("=", 1)[1].strip().strip('"')
        break


def run(cmd, **kwargs):
    return subprocess.run(cmd, cwd=ROOT, text=True, capture_output=True, **kwargs)


def row(results, name, status, detail=""):
    results.append({"id": name, "status": status, "detail": detail, "class": "software"})
    mark = "PASS" if status == "pass" else ("SKIP" if status == "skip" else "FAIL")
    print(f"[{mark}] {name}" + (f" — {detail}" if detail else ""))


def main():
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    results = []
    started = datetime.now(timezone.utc).isoformat()
    fast = os.environ.get("DA_QUALIFY_FAST", "") in ("1", "true", "yes")

    if not (ROOT / "profiles/minewing-gw1-r1.toml").is_file():
        row(results, "minewing_profile", "fail", "profiles/minewing-gw1-r1.toml missing")
    else:
        row(results, "minewing_profile", "pass")

    proc = run(["cargo", "fmt", "--all", "--", "--check"], timeout=120)
    row(results, "cargo_fmt", "pass" if proc.returncode == 0 else "fail", (proc.stdout + proc.stderr)[-300:])

    if fast:
        row(results, "clippy_default", "skip", "DA_QUALIFY_FAST=1 — covered by CI rust-native")
        row(results, "unit_tests", "skip", "DA_QUALIFY_FAST=1 — covered by CI rust-native")
    else:
        proc = run(["cargo", "clippy", "--all-targets", "--", "-D", "warnings"], timeout=600)
        row(results, "clippy_default", "pass" if proc.returncode == 0 else "fail", (proc.stdout + proc.stderr)[-400:])
        proc = run(["cargo", "test", "--all"], timeout=600)
        row(results, "unit_tests", "pass" if proc.returncode == 0 else "fail", (proc.stdout + proc.stderr)[-400:])

    proc = run(["cargo", "test", "--lib", "nodra::tests::"], timeout=180)
    out = proc.stdout + proc.stderr
    names = [
        "mqtt_transport_disabled_is_plain",
        "mqtt_transport_enabled_uses_default_roots",
        "nodra_tls_deserializes_from_toml",
    ]
    missing = [n for n in names if f"{n} ... ok" not in out]
    if proc.returncode == 0 and not missing:
        row(results, "nodra_mqtts_config", "pass", "3 MQTTS unit tests")
    else:
        row(
            results,
            "nodra_mqtts_config",
            "fail",
            (f"missing={missing}; " + out)[-400:],
        )

    for name, detail in [
        ("hardware_hil_minewing", "run scripts/hil/run-minewing-hil.sh + DA_HIL_SIGN=1 — docs/HIL.md"),
    ]:
        # Promote to pass when a claimable physical HIL run exists.
        hil_root = EVIDENCE / "hil"
        claimable = False
        stamp = ""
        if hil_root.is_dir():
            for p in sorted(hil_root.glob("*/results.json"), reverse=True):
                try:
                    data = json.loads(p.read_text())
                except Exception:
                    continue
                if data.get("minewing_claimable"):
                    claimable = True
                    stamp = p.parent.name
                    break
        if name == "hardware_hil_minewing" and claimable:
            row(results, name, "pass", f"signed hil/{stamp}")
        else:
            row(results, name, "skip", detail)

    arm64_pkg = os.environ.get("DA_ARM64_PACKAGES", "")
    if arm64_pkg in ("1", "true", "pass", "yes"):
        row(
            results,
            "arm64_native_deb_rpm",
            "pass",
            "CI packages-arm64 + release matrix ubuntu-24.04-arm",
        )
    else:
        row(
            results,
            "arm64_native_deb_rpm",
            "skip",
            "set DA_ARM64_PACKAGES=1 after packages-arm64 CI / release",
        )

    # Emulator CI rows: pass when CI env markers are set (jobs write these),
    # otherwise skip with a pointer to scripts/emulator/.
    emu_rows = [
        ("emulator_vcan_ci", "DA_EMULATOR_VCAN", "scripts/emulator/smoke-vcan.sh + CI emulator-vcan"),
        ("emulator_swtpm_ci", "DA_EMULATOR_SWTPM", "scripts/emulator/smoke-swtpm.sh + CI emulator-swtpm"),
        ("emulator_v4l2_ci", "DA_EMULATOR_V4L2", "scripts/emulator/smoke-v4l2.sh + CI emulator-v4l2 (soft-skip OK)"),
    ]
    for name, env_key, detail in emu_rows:
        val = os.environ.get(env_key, "")
        if val in ("1", "true", "pass", "yes"):
            row(results, name, "pass", detail)
        elif val in ("skip", "soft-skip"):
            row(results, name, "skip", f"{detail} — host reported soft-skip")
        else:
            row(results, name, "skip", f"set {env_key}=1 after smoke; {detail}")


    report = {
        "generated_at": started,
        "finished_at": datetime.now(timezone.utc).isoformat(),
        "product": "zyvor-device-agent",
        "version": VERSION,
        "host": os.uname().sysname if hasattr(os, "uname") else "unknown",
        "results": results,
        "software_pass": all(r["status"] == "pass" for r in results if r["status"] != "skip"),
        "ops_claimed": False,
        "note": "Hardware rows remain skip until Minewing GW1 r1 HIL is signed.",
    }
    out = EVIDENCE / "software-matrix.json"
    out.write_text(json.dumps(report, indent=2) + "\n")
    print(f"\nwrote {out}")
    if not report["software_pass"]:
        sys.exit(1)
    print("software qualification rows passed; hardware checklist still required for production")


if __name__ == "__main__":
    main()
