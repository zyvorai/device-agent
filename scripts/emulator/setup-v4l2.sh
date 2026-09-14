#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Load v4l2loopback and optionally feed a test pattern with ffmpeg.
set -euo pipefail

DEV=${ZYVOR_V4L2_DEV:-/dev/video0}
CARD=${ZYVOR_V4L2_CARD:-zyvor-loopback}

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "SKIP: v4l2loopback requires Linux" >&2
  exit 2
fi

if ! command -v modprobe >/dev/null; then
  echo "FAIL: modprobe required" >&2
  exit 1
fi

sudo modprobe v4l2loopback devices=1 video_nr=0 card_label="$CARD" exclusive_caps=1 \
  || { echo "FAIL: modprobe v4l2loopback failed (install v4l2loopback-dkms?)" >&2; exit 1; }

# Wait for node
for _ in $(seq 1 20); do
  [[ -e "$DEV" ]] && break
  sleep 0.1
done
[[ -e "$DEV" ]] || { echo "FAIL: $DEV missing after modprobe" >&2; exit 1; }

if command -v ffmpeg >/dev/null; then
  # Keep a low-rate test pattern running in the background.
  ffmpeg -loglevel error -re -f lavfi -i testsrc=size=320x240:rate=5 \
    -f v4l2 -pix_fmt yuyv422 "$DEV" >/tmp/zyvor-v4l2-ffmpeg.log 2>&1 &
  echo $! >/tmp/zyvor-v4l2-ffmpeg.pid
  sleep 0.5
else
  echo "WARN: ffmpeg not installed — device node only, no pattern feed"
fi

echo "v4l2loopback ready: $DEV"
ls -l "$DEV"
