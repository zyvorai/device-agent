# Security Policy

Do not open public issues for suspected security vulnerabilities. Report security issues privately to the Zyvor security contact published at https://zyvor.dev.

## v0.1 security boundaries

- The daemon is intended to run on a trusted Linux edge node.
- Physical bus access is local-only through the agent and explicit plugins.
- Plugin manifests must be provisioned by an administrator; arbitrary remote command execution is not a feature.
- Fleet tokens and Nodra credentials should be supplied through protected configuration or environment injection.
- The REST listener should be firewalled or bound to localhost until authentication is enabled for a deployment.
