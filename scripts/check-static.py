#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0

from __future__ import annotations

import json
import pathlib
import subprocess
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]


def main() -> int:
    for path in sorted((ROOT / "config").glob("*.toml")) + sorted((ROOT / "profiles").glob("*.toml")):
        with path.open("rb") as handle:
            tomllib.load(handle)
        print(f"TOML OK  {path.relative_to(ROOT)}")

    for path in sorted((ROOT / "profiles").glob("*/profile.toml")):
        with path.open("rb") as handle:
            tomllib.load(handle)
        print(f"TOML OK  {path.relative_to(ROOT)}")

    json_paths = [
        ROOT / "web/dashboard/package.json",
        ROOT / "docs/observability/grafana-dashboard.json",
        ROOT / "registry/verified-hardware.json",
        ROOT / "sdk/plugin-manifest.schema.json",
        *sorted((ROOT / "examples/plugins.d").glob("*.json")),
        *sorted((ROOT / "fixtures").rglob("*.json")),
    ]
    for path in json_paths:
        json.loads(path.read_text())
        print(f"JSON OK  {path.relative_to(ROOT)}")

    subprocess.run([sys.executable, str(ROOT / "examples/i2c_temperature.py"), "--self-test"], check=True)

    for script in sorted((ROOT / "scripts").glob("*.sh")) + sorted((ROOT / "scripts").rglob("*.sh")):
        subprocess.run(["bash", "-n", str(script)], check=True)
        print(f"SH OK    {script.relative_to(ROOT)}")

    smoke = ROOT / "examples/enrollment-server/smoke_test.py"
    if smoke.exists():
        subprocess.run([sys.executable, str(smoke)], check=False)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
