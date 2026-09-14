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

    proc = run(["cargo", "test", "--lib", "integrations::nodra::tests"], timeout=180)
    out = proc.stdout + proc.stderr
    passed = 0
    for line in out.splitlines():
        if line.startswith("test result: ok."):
            try:
                passed = int(line.split("ok.", 1)[1].split("passed", 1)[0].strip())
            except (IndexError, ValueError):
                passed = 0
    if proc.returncode == 0 and passed >= 3:
        row(results, "nodra_mqtts_config", "pass", f"{passed} tests")
    else:
        row(results, "nodra_mqtts_config", "fail", out[-400:])

    for name, detail in [
        ("hardware_hil_minewing", "operator-signed — evidence/qualification/hardware-checklist.md"),
        ("arm64_native_deb_rpm", "supported path today: arm64 tarball + multi-arch container; amd64 .deb/.rpm"),
        ("vcan_camera_swtpm_ci", "emulator CI jobs not yet wired"),
    ]:
        row(results, name, "skip", detail)

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
