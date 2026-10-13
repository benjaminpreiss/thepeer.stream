#!/usr/bin/env bash
# Current host integration entry point. No SDK/NDK/device work.
set -euo pipefail
cd "$(dirname "$0")"
bash test-scanner.sh
bash test-routing-native.sh
node --test tests/native-stage.test.mjs tests/application-fixture.test.mjs
bash -n native-build.sh
