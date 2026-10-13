# Build thepeer.stream from source

Run commands from the repository root unless stated otherwise. Rust builds use
locked dependencies and reviewed patches; use the scripts, not an unpatched
`cargo build` at the repository root.

## Prerequisites

- macOS or Linux with Git, Bash, Python 3.9+, Perl (`shasum`), `tar`, a C/C++
  compiler, Make and CMake. Android's NDK script supports macOS and x86_64 Linux.
- [Rustup](https://rustup.rs/). The commands below select the CI toolchain.
- [Node.js 24](https://nodejs.org/), used for staging and the example application.
- For Android: [Android Studio](https://developer.android.com/studio), JDK 17,
  Android SDK Platform 35, Build Tools 35.0.0 and NDK 27.2.12479018. Install these
  with Android Studio's SDK Manager; enable “Show Package Details” to select the NDK.
- An arm64 Android 8.0/API 26+ phone, a browser and USB debugging for developer installation.

On macOS, install Xcode Command Line Tools (`xcode-select --install`) and CMake.
On Debian/Ubuntu, native prerequisites can be installed with:

```sh
sudo apt-get update
sudo apt-get install build-essential clang cmake pkg-config perl python3 curl
```

Install Rustup and Node separately. Initial Cargo/Gradle dependency fetching needs
Internet access; the prepared Rust build then runs locked/offline. No relay is
started or contacted by these builds.

## 1. Clone and fetch dependencies

```sh
git clone https://github.com/benjaminpreiss/thepeer.stream.git
cd thepeer.stream
export THEPEER_RUST_TOOLCHAIN=1.95.0
rustup toolchain install "$THEPEER_RUST_TOOLCHAIN" --profile minimal --component rustfmt
cargo +"$THEPEER_RUST_TOOLCHAIN" fetch --locked
```

Keep `THEPEER_RUST_TOOLCHAIN` set for subsequent build commands in this shell.

## 2. Build and run the desktop helper

```sh
bash scripts/build-helper.sh
```

The executable is `target/build/debug/thepeer` unless you set `CARGO_TARGET_DIR`.
Start your own localhost HTTP application, or run the disposable test application:

```sh
node tests/fixtures/application-app.mjs 3002
```

In another interactive terminal, from the repository root:

```sh
./target/build/debug/thepeer --target-port 3002
```

It displays a live pairing QR. Do not redirect, save or screenshot it. Keep the
helper and target application running. See [helper options](HOST-HELPER.md) for
interface selection and optional relay configuration.

## 3. Build and install the phone app

Set your actual SDK location and JDK 17 `JAVA_HOME` in the shell. Typical SDK paths:

```sh
# macOS:
export ANDROID_HOME="$HOME/Library/Android/sdk"
# On Linux, use this instead:
# export ANDROID_HOME="$HOME/Android/Sdk"
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/27.2.12479018"
rustup target add --toolchain "$THEPEER_RUST_TOOLCHAIN" aarch64-linux-android
bash apps/android-controller/native-build.sh
```

Then open `apps/android-controller/` in Android Studio, Sync, select your phone,
and perform a full **Run app**. This installs **thepeer (Debug)**, a locally
debug-signed build with application ID `stream.thepeer.debug` and a `-debug`
version suffix. It can coexist with the release app (`stream.thepeer`).
Alternatively, with the phone connected and USB debugging authorized:

```sh
bash apps/android-controller/gradlew -p apps/android-controller :app:assembleDebug
"$ANDROID_HOME/platform-tools/adb" install -r apps/android-controller/app/build/outputs/apk/debug/app-debug.apk
```

Native artifacts are intentionally not in Git. Rerun `native-build.sh` whenever
Gradle reports missing/stale native inputs; do not bypass receipt validation.
Debug and release builds have separate application IDs and can coexist. Android
Studio installs the debug app without replacing the release app. Updates within
either ID require the same signing key; debug keys from different development
machines may differ.

## 4. Connect

Use the phone app's **Connect** button to scan the terminal QR. If Android asks
for camera permission, grant it and tap Connect again. The phone app connects and
opens the tunneled web application in the browser. Start with both devices on the
same network. Tap **Stop** in the phone app to end access; `q` or Ctrl-C also stops
the computer helper. Restart the helper to create a fresh QR for another session.

## Tests

```sh
bash scripts/test-native.sh
python3 -m unittest discover -s tests/host -p '*_test.py'
node --test apps/android-controller/tests/native-stage.test.mjs apps/android-controller/tests/application-fixture.test.mjs tests/host/repository-layout.test.mjs
```

The scanner/JNI suite additionally needs JDK 17 and its checksum-pinned ZXing jar:

```sh
mkdir -p apps/android-controller/.local-test/scanner
curl --fail --location --proto '=https' --tlsv1.2 \
  https://repo.maven.apache.org/maven2/com/google/zxing/core/3.5.3/core-3.5.3.jar \
  -o apps/android-controller/.local-test/scanner/core-3.5.3.jar
echo '8d8064c1636fdaef7189dd9055c7d59950a8940a12f2293956446ec3c109fd82  apps/android-controller/.local-test/scanner/core-3.5.3.jar' | shasum -a 256 --check
bash apps/android-controller/test-native.sh
```

Host checks do not substitute for [physical testing](HUMAN-TESTING.md).
See [releasing](RELEASING.md) for signed APKs and packaged helper binaries.
