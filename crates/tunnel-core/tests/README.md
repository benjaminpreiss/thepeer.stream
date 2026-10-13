# Test layout

- `internal/`: private-module tests and fixtures, included from `src/` with
  `#[cfg(test)]` and `#[path]`. They retain access to private implementation details
  without widening the library interface. Cargo does not auto-discover these nested
  files as separate integration-test crates.
- Public-interface integration tests belong directly under `tests/` when added or
  migrated; larger suites can use a single entry point and nested support modules.
- Small unit tests may remain inline beside the code they exercise.

`internal/routing.rs` collects the active Direct/Relay routing suites. Retired
coordination/diagnostic suites are removed; retained tests use the production
Direct/Relay entry point. `test_transport.rs` is a test-only raw QUIC fixture for
HTTP policy/ALPN cases, not another shipped transport or a MASQUE implementation.

Run `bash scripts/test-native.sh` from the repository root. The runner prepares
the exact reviewed transport patches and tests the workspace offline. A plain
unpatched Cargo build cannot supply those transport hooks. Host JNI/terminal QR
fixtures live separately under the repository's `tests/host/` directory.
