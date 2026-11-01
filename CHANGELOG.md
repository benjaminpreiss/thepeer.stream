# Validation history

Checks below describe specific builds and routes, not universal reachability or a validated release. For a repeatable device checklist, see [physical testing](docs/HUMAN-TESTING.md).

## Unreleased — terminal-to-terminal sharing

- A macOS CLI connected to an OrbStack Linux VM sharing a localhost web app over Direct. Browsing worked before and after a dev-server restart.
- The Android QR path was also checked.
- Host PTY/HTTP-restart and scanner/JNI fixtures were exercised. Host checks do not validate signed assets.
- **Still to verify:** terminal-to-terminal Relay, packaged CLIs on supported platforms, and the signed APK/release workflow.

## Earlier Android preview (pre-workspace cleanup)

- Same-Wi-Fi Direct with automatic binding, cellular single-carrier Relay and Stop on both routes were confirmed on the `0.6-direct-relay-preview` build. Cellular Relay felt slow; no benchmark was recorded.
- These checks do not validate a later Android build or release.
