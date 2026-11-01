# Validation record

These are build-specific checks, not universal reachability or release claims.
For a repeatable checklist, see [physical testing](HUMAN-TESTING.md).

## Unreleased — terminal-to-terminal sharing

- Direct: macOS CLI to OrbStack Linux VM sharing a localhost web app; browsing
  worked before and after a dev-server restart. The Android QR path was checked.
- Host PTY/HTTP-restart and scanner/JNI fixtures were exercised.
- **Still to verify:** desktop Relay, packaged CLIs, signed APK and hosted
  release workflow.

## Earlier Android preview (pre-workspace cleanup)

- `0.6-direct-relay-preview`: same-Wi-Fi Direct with automatic binding,
  cellular single-carrier Relay and Stop on both routes. Cellular Relay felt
  slow; no benchmark was recorded.
- These checks do not validate a later Android build or release.
