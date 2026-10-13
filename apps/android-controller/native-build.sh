#!/usr/bin/env bash
# Run by the operator OUTSIDE the agent sandbox. No downloads or device commands.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
# Preserve the existing NDK cache; sources now belong to the root workspace.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/native/target}"
export CARGO_INCREMENTAL=0
TOOLCHAIN="${THEPEER_RUST_TOOLCHAIN:-stable}"
if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
  SDK="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/Library/Android/sdk}}"
  ANDROID_NDK_HOME="$(node -e '
    const fs = require("node:fs"), path = require("node:path");
    const root = path.join(process.argv[1], "ndk");
    try {
    const versions = fs.readdirSync(root).filter(v => /^\d+\.\d+\.\d+$/.test(v))
      .sort((a,b) => a.localeCompare(b, "en", {numeric:true}));
    if (!versions.length) process.exit(1);
    process.stdout.write(path.join(root, versions.at(-1)));
    } catch { process.exit(1); }
  ' "$SDK")" || { echo 'Install NDK (Side by side) in Android Studio, or set ANDROID_NDK_HOME.' >&2; exit 1; }
fi
export ANDROID_NDK_HOME
export ANDROID_NDK="$ANDROID_NDK_HOME"
export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"
case "$(uname -s)" in
  Darwin) HOST=darwin-x86_64 ;;
  Linux) HOST=linux-x86_64 ;;
  *) echo 'This build script supports macOS/Linux hosts only.' >&2; exit 1 ;;
esac
BIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$HOST/bin"
CLANG="$BIN/aarch64-linux-android26-clang"
[[ -x "$CLANG" && -x "$BIN/llvm-strip" ]] || { echo 'NDK toolchain missing; check ANDROID_NDK_HOME.' >&2; exit 1; }
rustup target list --installed --toolchain "$TOOLCHAIN" | grep -qx aarch64-linux-android || {
  echo "First run: rustup target add --toolchain $TOOLCHAIN aarch64-linux-android" >&2; exit 1;
}
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CLANG"
export CC_aarch64_linux_android="$CLANG"
export AR_aarch64_linux_android="$BIN/llvm-ar"
export CFLAGS_aarch64_linux_android="-ffile-prefix-map=$HOME=/private-home -fdebug-prefix-map=$HOME=/private-home"
export CARGO_ENCODED_RUSTFLAGS
CARGO_ENCODED_RUSTFLAGS="$(printf '%s\037%s\037%s' \
  "--remap-path-prefix=$HOME=/private-home" \
  "--remap-path-prefix=$ROOT=/controller-probe" \
  '-Clink-arg=-Wl,-z,max-page-size=16384')"
mkdir -p "$CARGO_TARGET_DIR"
INPUT_DIR=$(mktemp -d "$CARGO_TARGET_DIR/build-inputs.XXXXXX")
trap 'rm -f "$INPUT_DIR/source.json"; rmdir "$INPUT_DIR"' EXIT
node "$ROOT/native-stage.mjs" --snapshot "$INPUT_DIR/source.json"
NATIVE_SOURCE=$(bash "$ROOT/../../patches/saorsa-transport/prepare-native.sh")
echo 'Building pinned, patched routing native transport for arm64-v8a / Android API 26 (offline, locked).'
cargo "+$TOOLCHAIN" build --locked --offline --release --lib -p tunnel-android --target aarch64-linux-android \
  --manifest-path "$NATIVE_SOURCE/Cargo.toml"
LIB="$CARGO_TARGET_DIR/aarch64-linux-android/release/libcontroller_saorsa_probe.so"
"$BIN/llvm-strip" --strip-debug "$LIB"
for method in nativeRouteSupported nativeRouteRun nativeRouteKind nativeViewerPrepare nativeViewerStop nativeViewerAbandonPrepared nativeViewerPort; do
  "$BIN/llvm-nm" --dynamic --defined-only "$LIB" | grep "Java_dev_webp2p_controllerprobe_NativeSaorsaProbe_${method}\$" >/dev/null
done
"$BIN/llvm-nm" --dynamic --defined-only "$LIB" | grep 'web_p2p_routing_client_v2$' >/dev/null
node "$ROOT/native-stage.mjs" "$LIB" "$INPUT_DIR/source.json"
echo 'Native probe staged. In Android Studio: Sync Gradle, then full Run app.'
