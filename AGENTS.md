# Repository guidance

- Preserve the product boundary: Device Agent discovers hardware; Nodra interprets protocols; Fleet owns desired state; Aether owns workload/runtime placement.
- Do not add a Kubernetes runtime dependency to the daemon.
- Prefer Linux kernel/sysfs interfaces and small, auditable dependencies.
- Hardware-specific behavior belongs in board profiles or plugins, not generic discovery code.
- New industrial protocols belong in Nodra unless they are strictly physical-bus discovery.
- Every new parser needs a deterministic fixture/unit test.
- The dashboard must work without CDN assets, trackers or external fonts.
- Zyvor orange is an accent, not a background theme.
