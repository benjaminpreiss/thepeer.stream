#!/usr/bin/env bash
# Supported workspace tests, using only local network fixtures; no APK/NDK work.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target/build}"
export CARGO_INCREMENTAL=0
native=$(bash "$root/scripts/native-workspace.sh")
cargo "+${THEPEER_RUST_TOOLCHAIN:-stable}" test --locked --offline --manifest-path "$native/Cargo.toml" --workspace -- --test-threads=2
