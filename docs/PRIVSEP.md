# Privilege separation: bus-access helper

With `[privsep].enabled = false` (the default) the API process opens GPIO/I²C/
SPI/CAN/camera nodes directly and the stock systemd unit stays `User=root`.

With `enabled = true` and a binary built with `--features privsep`, `serve`
spawns `bus-helper` (or re-executes this binary's `bus-helper` subcommand) and
talks to it over an authenticated Unix socket. Peer credentials are checked
with `SO_PEERCRED` / `getpeereid`. Unknown opcodes and unallowlisted peers are
rejected. There is no shell and no arbitrary exec.

## Operations

| Opcode | Purpose |
|---|---|
| `inventory` | Full inventory snapshot |
| `doctor` | Doctor report inputs |
| `can-rx` | One receive-only read on an allowlisted CAN interface |
| `camera-read` | Open an allowlisted `/dev/video*` node |

## Migration

1. Build with `--features privsep`.
2. Set `[privsep].enabled = true`.
3. Install `packaging/systemd/zyvor-device-agent.privsep.service.example` if the
   API process should run as `User=zyvor`.
4. Keep the stock root unit until the helper path is validated on the board.

See also `docs/HARDWARE_PERMISSIONS.md`.
