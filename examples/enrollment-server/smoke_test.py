# SPDX-License-Identifier: Apache-2.0
"""Smoke-test the lab enrollment fixture against the CSR JSON protocol."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SERVER = ROOT / "examples/enrollment-server/server.py"


def main() -> int:
    try:
        from cryptography.hazmat.primitives.asymmetric import ec  # noqa: F401
    except ImportError:
        print("cryptography not installed; skipping enrollment server smoke")
        return 0
    with tempfile.TemporaryDirectory() as tmp:
        state = Path(tmp)
        proc = subprocess.Popen(
            [sys.executable, str(SERVER), "--listen", "127.0.0.1:0", "--state", str(state)],
            stdout=subprocess.PIPE,
            text=True,
        )
        # The server prints listen info; for port 0 we need a fixed port.
        proc.kill()
        proc = subprocess.Popen(
            [sys.executable, str(SERVER), "--listen", "127.0.0.1:19443", "--state", str(state)],
            stdout=subprocess.PIPE,
            text=True,
        )
        time.sleep(0.5)
        token = (state / "token").read_text().strip()
        # Build a throwaway CSR with openssl if available; otherwise skip deep path.
        try:
            subprocess.run(
                ["openssl", "req", "-new", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
                 "-nodes", "-keyout", str(state / "key.pem"), "-out", str(state / "csr.pem"),
                 "-subj", "/CN=lab-device"],
                check=True,
                capture_output=True,
            )
        except (FileNotFoundError, subprocess.CalledProcessError):
            proc.kill()
            print("openssl unavailable; enrollment server module loads")
            return 0
        csr = (state / "csr.pem").read_text()
        req = urllib.request.Request(
            "http://127.0.0.1:19443/enroll",
            data=json.dumps({"csr_pem": csr, "common_name": "lab-device"}).encode(),
            headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=5) as resp:
            body = json.load(resp)
            assert "certificate_pem" in body
        # second use of the same token must fail (single-use)
        try:
            urllib.request.urlopen(req, timeout=5)
            raise AssertionError("token should be single-use")
        except Exception as error:
            if "AssertionError" in type(error).__name__:
                raise
        proc.kill()
        print("enrollment server smoke ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
