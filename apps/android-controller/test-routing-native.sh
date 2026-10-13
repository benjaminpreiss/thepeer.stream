#!/usr/bin/env bash
# Host JNI integration, real helper terminal/QR and browser. No APK/NDK/device work.
set -euo pipefail
[[ $# == 0 ]] || { echo 'Usage: test-routing-native.sh (loopback only)' >&2; exit 2; }
root=$(cd "$(dirname "$0")/../.." && pwd)
: "${JAVA_HOME:?Set JAVA_HOME to an installed host JDK}"
jar=${ZXING_CORE_JAR:-"$root/apps/android-controller/.local-test/scanner/core-3.5.3.jar"}
python3 - "$jar" <<'PY'
import hashlib,sys
from pathlib import Path
p=Path(sys.argv[1])
if not p.is_file() or hashlib.sha256(p.read_bytes()).hexdigest()!='8d8064c1636fdaef7189dd9055c7d59950a8940a12f2293956446ec3c109fd82':
    raise SystemExit('Supply pinned ZXing core 3.5.3; no downloads performed')
PY
native=$(bash "$root/scripts/native-workspace.sh")
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target/build}"
export CARGO_INCREMENTAL=0
cargo "+${THEPEER_RUST_TOOLCHAIN:-stable}" build --locked --offline --manifest-path "$native/Cargo.toml" -p tunnel-android -p thepeer-cli
classes="$native/target/routing-jni/classes"
mkdir -p "$classes"
"$JAVA_HOME/bin/javac" -cp "$jar" -d "$classes" \
    "$root/tests/host/HelperQrDecode.java" \
    "$root/apps/android-controller/app/src/main/java/dev/webp2p/controllerprobe/NativeSaorsaProbe.java" \
    "$root/apps/android-controller/app/src/main/java/dev/webp2p/controllerprobe/ViewerLaunch.java" \
    "$root/apps/android-controller/app/src/main/java/dev/webp2p/controllerprobe/ViewerScan.java" \
    "$root/apps/android-controller/tests/RoutingPhoneFixture.java"
actor="$native/target/routing-jni/phone"
printf '#!/usr/bin/env bash\nexec %q %q -cp %q dev.webp2p.controllerprobe.RoutingPhoneFixture\n' \
    "$JAVA_HOME/bin/java" "-Djava.library.path=$CARGO_TARGET_DIR/debug" "$classes" > "$actor"
chmod +x "$actor"
python3 "$root/tests/host/helper_process_tests.py" \
    "$CARGO_TARGET_DIR/debug/thepeer" "$actor" "$JAVA_HOME/bin/java" "$classes:$jar"
