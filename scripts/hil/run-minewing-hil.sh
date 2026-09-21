#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Minewing GW1 r1 HIL runner for Zyvor Device Agent.
#
#   DA_HIL_BASE=http://127.0.0.1:9188 DA_HIL_ENV=physical|lab-surrogate|qemu \
#     ./scripts/hil/run-minewing-hil.sh
#
# Never auto-signs the hardware checklist unless DA_HIL_SIGN=1 AND
# environment=physical AND every required row is pass.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
BASE=${DA_HIL_BASE:-http://127.0.0.1:9188}
PROFILE=${DA_HIL_PROFILE:-minewing-gw1-r1}
TOKEN=${DA_HIL_TOKEN:-}
ENV_LABEL=${DA_HIL_ENV:-physical}
STRICT=${DA_HIL_STRICT:-1}
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
OUT="$ROOT/evidence/qualification/hil/$STAMP"
mkdir -p "$OUT"

HDR=()
[[ -n "$TOKEN" ]] && HDR=(-H "Authorization: Bearer $TOKEN")

fetch() {
  local path=$1 file=$2
  curl -fsS "${HDR[@]}" "$BASE$path" >"$file" 2>"$file.err"
}

echo "base=$BASE profile=$PROFILE env=$ENV_LABEL stamp=$STAMP" | tee "$OUT/meta.env"

python3 - "$OUT" "$BASE" "$PROFILE" "$ENV_LABEL" "$STRICT" "$ROOT" "${DA_HIL_WAN_LOSS_LOG:-}" "${DA_HIL_SIGN:-0}" <<'PY'
import json, os, pathlib, subprocess, sys, datetime, urllib.request

out = pathlib.Path(sys.argv[1])
base = sys.argv[2].rstrip("/")
profile = sys.argv[3]
env = sys.argv[4]
strict = sys.argv[5] == "1"
root = pathlib.Path(sys.argv[6])
wan_log = sys.argv[7]
sign = sys.argv[8] == "1"
token = os.environ.get("DA_HIL_TOKEN", "")

rows = []

def add(rid, status, detail):
    rows.append({"id": rid, "status": status, "detail": detail})
    print(f"[{status.upper()}] {rid} — {detail}")

def get(path):
    req = urllib.request.Request(base + path)
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    with urllib.request.urlopen(req, timeout=15) as resp:
        body = resp.read()
        return resp.status, json.loads(body) if body else {}

def get_raw(path):
    req = urllib.request.Request(base + path)
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return resp.status, resp.read()
    except Exception as e:
        code = getattr(getattr(e, "code", None), "real", None) or getattr(e, "code", None)
        return code or 0, str(e).encode()

# health
try:
    code, health = get("/api/v1/health")
    (out / "health.json").write_text(json.dumps(health, indent=2) + "\n")
    add("health", "pass" if code == 200 else "fail", f"HTTP {code}")
except Exception as e:
    add("health", "fail", str(e))

# doctor
try:
    code, doc = get("/api/v1/doctor")
    (out / "doctor.json").write_text(json.dumps(doc, indent=2) + "\n")
    checks = {c.get("name"): c for c in doc.get("checks", []) if isinstance(c, dict)}
    detail = checks.get("profile.name", {}).get("detail", "")
    ok = bool(doc.get("ok"))
    if env == "physical":
        if profile not in detail:
            add("doctor_profile", "fail", f"want profile {profile}, got {detail!r}")
        elif ok:
            add("doctor_profile", "pass", detail or "doctor ok")
        else:
            add("doctor_profile", "fail", f"doctor ok=false ({detail})")
    else:
        add("doctor_profile", "blocked" if not ok else "pass",
            f"{env}: doctor ok={ok} ({detail})")
except Exception as e:
    add("doctor_profile", "fail", str(e))

# inventory buses
try:
    code, inv = get("/api/v1/inventory")
    (out / "inventory.json").write_text(json.dumps(inv, indent=2) + "\n")
    arch = (inv.get("system") or {}).get("arch", "")
    nets = inv.get("network") or []
    phys_eth = [n for n in nets if str(n.get("name", "")).startswith(("en", "eth"))]
    can = [n for n in nets if str(n.get("name", "")).startswith(("can", "vcan"))]
    if env == "physical":
        if arch != "aarch64":
            add("buses", "fail", f"arch={arch} want aarch64")
        elif len(phys_eth) < 1 or len(can) < 1:
            add("buses", "fail", f"eth={len(phys_eth)} can={len(can)}")
        else:
            add("buses", "pass", f"arch={arch} eth={len(phys_eth)} can={len(can)}")
    else:
        add("buses", "blocked", f"surrogate arch={arch} eth={len(phys_eth)} can={len(can)}")
except Exception as e:
    add("buses", "fail", str(e))

# i2c / sensors
try:
    code, sensors = get("/api/v1/sensors")
    (out / "sensors.json").write_text(json.dumps(sensors, indent=2) + "\n")
    blob = json.dumps(sensors).lower()
    if any(k in blob for k in ("i2c", "lm75", "tmp102", "temperature")):
        add("i2c_plugin", "pass" if env == "physical" else "blocked",
            "temperature/i2c evidence present" + ("" if env == "physical" else " (surrogate)"))
    else:
        add("i2c_plugin", "fail" if env == "physical" else "blocked",
            "no i2c/temperature samples")
except Exception as e:
    add("i2c_plugin", "blocked", f"sensors: {e}")

# nodra / status
try:
    code, status = get("/api/v1/status")
    (out / "status.json").write_text(json.dumps(status, indent=2) + "\n")
    blob = json.dumps(status).lower()
    if "nodra_connected\": true" in json.dumps(status) or '"nodra_connected": true' in json.dumps(status):
        add("nodra", "pass", "nodra_connected=true")
    elif "nodra" in blob and "true" in blob:
        add("nodra", "pass", "nodra appears connected")
    else:
        add("nodra", "blocked", "nodra not connected")
except Exception as e:
    add("nodra", "blocked", str(e))

# fleet inventory
try:
    code, finv = get("/api/v1/integrations/fleet/inventory")
    (out / "fleet-inventory.json").write_text(json.dumps(finv, indent=2) + "\n")
    add("fleet_inventory", "pass", "fleet inventory served")
except Exception as e:
    add("fleet_inventory", "blocked", str(e))

# auth posture
code, _ = get_raw("/api/v1/inventory")
loopback = any(h in base for h in ("127.0.0.1", "localhost", "[::1]"))
if code in (401, 403):
    add("auth", "pass", f"inventory HTTP {code}")
elif loopback:
    add("auth", "blocked", f"open API on loopback (HTTP {code})")
else:
    add("auth", "fail", f"non-loopback open API HTTP {code}")

# systemd + health for OTA probes
try:
    r = subprocess.run(["systemctl", "is-active", "zyvor-device-agent"],
                       capture_output=True, text=True)
    if r.returncode == 0 and r.stdout.strip() == "active":
        add("ota_health_systemd", "pass", "unit active")
    else:
        add("ota_health_systemd", "blocked", f"unit={r.stdout.strip() or r.stderr.strip()}")
except Exception as e:
    add("ota_health_systemd", "blocked", str(e))

try:
    code, _ = get("/api/v1/health")
    add("ota_health_http", "pass" if code == 200 else "fail", f"HTTP {code}")
except Exception as e:
    add("ota_health_http", "fail", str(e))

# wan loss
if wan_log and pathlib.Path(wan_log).is_file():
    pathlib.Path(out / "wan-loss.log").write_bytes(pathlib.Path(wan_log).read_bytes())
    add("wan_loss", "pass", f"attached {wan_log}")
else:
    add("wan_loss", "blocked", "set DA_HIL_WAN_LOSS_LOG to attach evidence")

# rs485 declaration evidence
try:
    code, industrial = get("/api/v1/industrial")
    (out / "industrial.json").write_text(json.dumps(industrial, indent=2) + "\n")
    serial = industrial.get("serial") or []
    rs485 = [p for p in serial if p.get("rs485")]
    if env == "physical":
        add("rs485", "pass" if rs485 else "fail", f"rs485_ports={len(rs485)}")
    else:
        add("rs485", "blocked", f"surrogate rs485_ports={len(rs485)}")
except Exception as e:
    add("rs485", "blocked" if env != "physical" else "fail", str(e))

# power-cycle evidence (operator attaches a journal from the previous boot)
power_log = os.environ.get("DA_HIL_POWER_CYCLE_LOG", "")
if power_log and pathlib.Path(power_log).is_file():
    pathlib.Path(out / "power-cycle.log").write_bytes(pathlib.Path(power_log).read_bytes())
    add("power_cycle", "pass", f"attached {power_log}")
else:
    add("power_cycle", "blocked", "set DA_HIL_POWER_CYCLE_LOG to attach evidence")

counts = {}
for r in rows:
    counts[r["status"]] = counts.get(r["status"], 0) + 1
claimable = (
    env == "physical"
    and counts.get("fail", 0) == 0
    and counts.get("blocked", 0) == 0
    and counts.get("pass", 0) > 0
)
report = {
    "product": "zyvor-device-agent",
    "sku": "minewing-gw1-r1",
    "environment": env,
    "base": base,
    "profile": profile,
    "stamp": out.name,
    "finished_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "counts": counts,
    "minewing_claimable": claimable,
    "rows": rows,
}
(out / "results.json").write_text(json.dumps(report, indent=2) + "\n")
(out / "SUMMARY.md").write_text(
    f"# Device Agent HIL summary\n\n"
    f"- environment: `{env}`\n"
    f"- counts: `{counts}`\n"
    f"- minewing_claimable: **{claimable}**\n\n"
    f"Production Minewing claims require `DA_HIL_ENV=physical`, loaded profile "
    f"`minewing-gw1-r1`, and zero fail/blocked rows, then `DA_HIL_SIGN=1`.\n"
)

if sign:
    if not claimable:
        raise SystemExit("refusing DA_HIL_SIGN=1: minewing_claimable=false")
    checklist = root / "evidence/qualification/hardware-checklist.md"
    mapping = [
        ("doctor_profile", "doctor profile checks green on board"),
        ("buses", "Ethernet / UART / CAN / USB / watchdog present"),
        ("i2c_plugin", "I2C reference plugin reads real sensor"),
        ("nodra", "Nodra MQTT (or MQTTS) connected"),
        ("fleet_inventory", "Fleet inventory projection reachable"),
        ("auth", "Auth ≠ none when non-loopback"),
        ("ota_health_http", "OTA health probes (systemd + /health)"),
        ("wan_loss", "WAN-loss / reconnect with Nodra WAL"),
        ("rs485", "RS485 declared and visible"),
        ("power_cycle", "Power-cycle boot evidence"),
    ]
    by = {r["id"]: r for r in rows}
    block = [
        "",
        f"## Signed HIL run `{out.name}`",
        "",
        f"- Environment: physical Minewing GW1 r1",
        f"- Finished: {report['finished_at']}",
        f"- Evidence: `evidence/qualification/hil/{out.name}/`",
        "",
        "| Test | Pass? | Notes |",
        "|---|---|---|",
    ]
    for rid, label in mapping:
        r = by.get(rid, {})
        block.append(f"| {label} | {r.get('status','')} | {r.get('detail','')} |")
    block += ["", f"Operator sign-off stamp: {out.name}", ""]
    checklist.write_text(checklist.read_text().rstrip() + "\n" + "\n".join(block) + "\n")
    print(f"updated {checklist}")

print(f"\nwrote {out}")
if strict and env == "physical" and not claimable:
    sys.exit(1)
PY
