#!/usr/bin/env bash
# Offline desktop build. Does not start the helper or contact any participant.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target/build}"
export CARGO_INCREMENTAL=0
native=$(bash "$root/scripts/native-workspace.sh")
cargo "+${THEPEER_RUST_TOOLCHAIN:-stable}" build --locked --offline --manifest-path "$native/Cargo.toml" -p thepeer-cli
printf 'Helper: %s/debug/thepeer\n' "$CARGO_TARGET_DIR"
