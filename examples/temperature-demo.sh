#!/usr/bin/env sh
set -eu
printf '{"sensor":"demo-temperature","value":%s,"unit":"celsius","quality":"simulated"}\n' "${ZYVOR_DEMO_TEMP:-24.8}"
