#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Lab-only enrollment CA for Device Agent CSR protocol.

Fleet must run the production CA. This fixture signs CSRs for local tests and
CI. It is not a production certificate authority.
"""

from __future__ import annotations

import argparse
import json
import secrets
import threading
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.x509.oid import NameOID


class State:
    def __init__(self, root: Path) -> None:
        self.root = root
        self.root.mkdir(parents=True, exist_ok=True)
        self.tokens = {secrets.token_urlsafe(16)}
        self.revoked: set[str] = set()
        self.audit = self.root / "audit.log"
        key_path = self.root / "ca-key.pem"
        cert_path = self.root / "ca-cert.pem"
        if key_path.exists() and cert_path.exists():
            self.ca_key = serialization.load_pem_private_key(key_path.read_bytes(), password=None)
            self.ca_cert = x509.load_pem_x509_certificate(cert_path.read_bytes())
        else:
            self.ca_key = ec.generate_private_key(ec.SECP256R1())
            subject = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "zyvor-lab-enrollment-ca")])
            self.ca_cert = (
                x509.CertificateBuilder()
                .subject_name(subject)
                .issuer_name(subject)
                .public_key(self.ca_key.public_key())
                .serial_number(x509.random_serial_number())
                .not_valid_before(datetime.now(timezone.utc))
                .not_valid_after(datetime.now(timezone.utc) + timedelta(days=3650))
                .add_extension(x509.BasicConstraints(ca=True, path_length=None), critical=True)
                .sign(self.ca_key, hashes.SHA256())
            )
            key_path.write_bytes(
                self.ca_key.private_bytes(
                    serialization.Encoding.PEM,
                    serialization.PrivateFormat.PKCS8,
                    serialization.NoEncryption(),
                )
            )
            cert_path.write_bytes(self.ca_cert.public_bytes(serialization.Encoding.PEM))
        (self.root / "token").write_text(next(iter(self.tokens)) + "\n")

    def log(self, action: str, detail: dict) -> None:
        line = json.dumps({"action": action, **detail}) + "\n"
        with self.audit.open("a", encoding="utf-8") as handle:
            handle.write(line)


def make_handler(state: State):
    class Handler(BaseHTTPRequestHandler):
        def _read_json(self) -> dict:
            length = int(self.headers.get("Content-Length", "0"))
            raw = self.rfile.read(length) if length else b"{}"
            return json.loads(raw.decode() or "{}")

        def _json(self, code: int, body: dict) -> None:
            payload = json.dumps(body).encode()
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def do_GET(self) -> None:  # noqa: N802
            if self.path.startswith("/revocation"):
                from urllib.parse import parse_qs, urlparse

                query = parse_qs(urlparse(self.path).query)
                common_name = (query.get("common_name") or [""])[0]
                revoked = common_name in state.revoked
                state.log("revocation-check", {"common_name": common_name, "revoked": revoked})
                return self._json(200, {"revoked": revoked})
            if self.path == "/health":
                return self._json(200, {"ok": True, "tokens": len(state.tokens)})
            return self._json(404, {"error": "not found"})

        def do_POST(self) -> None:  # noqa: N802
            if self.path not in ("/enroll", "/"):
                return self._json(404, {"error": "not found"})
            auth = self.headers.get("Authorization", "")
            token = auth.removeprefix("Bearer ").strip()
            body = self._read_json()
            renew = bool(body.get("renew"))
            if token not in state.tokens and not renew:
                return self._json(401, {"error": "invalid token"})
            if not renew:
                state.tokens.discard(token)
            csr_pem = body.get("csr_pem", "")
            common_name = body.get("common_name", "device")
            csr = x509.load_pem_x509_csr(csr_pem.encode())
            cert = (
                x509.CertificateBuilder()
                .subject_name(x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, common_name)]))
                .issuer_name(state.ca_cert.subject)
                .public_key(csr.public_key())
                .serial_number(x509.random_serial_number())
                .not_valid_before(datetime.now(timezone.utc))
                .not_valid_after(datetime.now(timezone.utc) + timedelta(days=365))
                .sign(state.ca_key, hashes.SHA256())
            )
            state.log("issue" if not renew else "renew", {"common_name": common_name})
            return self._json(
                200,
                {
                    "certificate_pem": cert.public_bytes(serialization.Encoding.PEM).decode(),
                    "ca_bundle_pem": state.ca_cert.public_bytes(serialization.Encoding.PEM).decode(),
                },
            )

        def log_message(self, format: str, *args) -> None:  # noqa: A003
            return

    return Handler


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--listen", default="127.0.0.1:9443")
    parser.add_argument("--state", default="/tmp/zyvor-lab-enrollment")
    args = parser.parse_args()
    host, port_s = args.listen.rsplit(":", 1)
    state = State(Path(args.state))
    server = ThreadingHTTPServer((host, int(port_s)), make_handler(state))
    print(json.dumps({"listen": args.listen, "token_file": str(Path(args.state) / "token")}))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        thread.join()
    except KeyboardInterrupt:
        server.shutdown()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
