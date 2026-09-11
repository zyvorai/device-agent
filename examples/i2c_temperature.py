#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Minimal I2C temperature reference plugin for Zyvor Device Agent.

The plugin performs an explicit read from one configured bus/address. It never
scans the I2C bus, because active probing can have side effects on industrial
hardware. Supported register formats: LM75-compatible and TMP102-compatible.
"""

from __future__ import annotations

import argparse
import fcntl
import json
import os
import sys
import time

I2C_SLAVE = 0x0703
TEMP_REGISTER = 0x00


def decode_lm75(msb: int, lsb: int) -> float:
    value = ((msb << 8) | lsb) >> 7
    if value & 0x100:
        value -= 0x200
    return value * 0.5


def decode_tmp102(msb: int, lsb: int) -> float:
    value = ((msb << 8) | lsb) >> 4
    if value & 0x800:
        value -= 0x1000
    return value * 0.0625


def read_temperature(bus: str, address: int, sensor: str) -> float:
    fd = os.open(bus, os.O_RDWR | os.O_CLOEXEC)
    try:
        fcntl.ioctl(fd, I2C_SLAVE, address)
        os.write(fd, bytes([TEMP_REGISTER]))
        raw = os.read(fd, 2)
        if len(raw) != 2:
            raise RuntimeError(f"short I2C read: expected 2 bytes, received {len(raw)}")
        if sensor == "lm75":
            return decode_lm75(raw[0], raw[1])
        if sensor == "tmp102":
            return decode_tmp102(raw[0], raw[1])
        raise RuntimeError(f"unsupported sensor format: {sensor}")
    finally:
        os.close(fd)


def self_test() -> None:
    cases = [
        ("lm75 +25.5C", decode_lm75(0x19, 0x80), 25.5),
        ("lm75 -10.5C", decode_lm75(0xF5, 0x80), -10.5),
        ("tmp102 +25.0625C", decode_tmp102(0x19, 0x10), 25.0625),
        ("tmp102 -10.0C", decode_tmp102(0xF6, 0x00), -10.0),
    ]
    for name, actual, expected in cases:
        if abs(actual - expected) > 1e-9:
            raise AssertionError(f"{name}: expected {expected}, received {actual}")
    print("i2c_temperature self-test: PASS")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bus", default="/dev/i2c-1")
    parser.add_argument("--address", default="0x48")
    parser.add_argument("--sensor", choices=["lm75", "tmp102"], default="lm75")
    parser.add_argument("--sensor-id", default="cabinet-temperature")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return 0

    address = int(args.address, 0)
    if not 0x03 <= address <= 0x77:
        raise ValueError("I2C address must be in the normal 7-bit device range 0x03..0x77")

    temperature = read_temperature(args.bus, address, args.sensor)
    payload = {
        "sensor": args.sensor_id,
        "value": temperature,
        "kind": "temperature",
        "unit": "celsius",
        "quality": "good",
        "timestamp_unix_ms": int(time.time() * 1000),
        "labels": {
            "bus": args.bus,
            "address": hex(address),
            "sensor_family": args.sensor,
        },
    }
    print(json.dumps(payload, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:  # plugin contract: nonzero + stderr on failure
        print(f"i2c_temperature: {exc}", file=sys.stderr)
        raise SystemExit(2)
