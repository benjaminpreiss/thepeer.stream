#!/usr/bin/env bash
# Offline host checks; no SDK, live capabilities, downloads or legacy helper.
set -euo pipefail
cd "$(dirname "$0")"
: "${JAVA_HOME:?Set JAVA_HOME to a JDK 17+}"
jar=${ZXING_CORE_JAR:-"$PWD/.local-test/scanner/core-3.5.3.jar"}
node --input-type=module -e '
  import {readFileSync} from "node:fs"; import {createHash} from "node:crypto";
  if (createHash("sha256").update(readFileSync(process.argv[1])).digest("hex") !== "8d8064c1636fdaef7189dd9055c7d59950a8940a12f2293956446ec3c109fd82") throw new Error("Supply pinned ZXing core 3.5.3; no downloads performed");
' "$jar"
classes=.local-test/routing-scanner
mkdir -p "$classes"
"$JAVA_HOME/bin/javac" --release 17 -cp "$jar" -d "$classes" \
  app/src/main/java/dev/webp2p/controllerprobe/{RelayQrDecoder,ViewerLaunch,ViewerScan,ConnectionState,ConnectionHandoff}.java \
  tests/{ViewerLaunchTest,ViewerScanTest,ConnectionStateTest,ConnectionHandoffTest}.java
for test in ViewerLaunchTest ViewerScanTest ConnectionStateTest ConnectionHandoffTest; do
  "$JAVA_HOME/bin/java" -cp "$classes:$jar" "dev.webp2p.controllerprobe.$test"
done
