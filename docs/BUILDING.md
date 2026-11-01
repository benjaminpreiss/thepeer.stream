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

## 2. Build and run the CLI

```sh
bash scripts/build-helper.sh
```

The executable is `target/build/debug/thepeer` unless you set `CARGO_TARGET_DIR`.
Start your own localhost HTTP application, or run this minimal server in one terminal
(no Node dependencies beyond the prerequisite runtime):

```sh
node -e 'require("node:http").createServer((_req, res) => res.end("hello from localhost\n")).listen(3002, "127.0.0.1")'
```

For the richer disposable test application instead, run:

```sh
node tests/fixtures/application-app.mjs 3002
```

In another interactive terminal, from the repository root:

```sh
./target/build/debug/thepeer share localhost:3002
```

The sharing CLI shows text by default; `r` shows its QR and `t` returns to text.
Keep it running, and keep either form of the offer out of files and screenshots.
See [CLI options](CLI.md) for bind and Relay configuration.

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

## 4. Open the app

Use the same release or source revision on both devices. Start on the same network
for Direct; see [Relay options](CLI.md#build-and-run) for an explicitly configured
participant.

### Another computer (including an SSH host sharing to a laptop)

On the browsing computer, run:

```sh
./target/build/debug/thepeer connect
```

Paste the text offer at the **hidden** prompt; open the printed loopback URL in
that computer's browser. Never pass the offer in a command or log it.

### Android phone

Press `r` on the sharing computer. Tap **Connect** in the Android app and scan
its QR. If asked for camera access, grant it and tap Connect again.

### End or resume

The first peer wins the one-shot offer. Use Ctrl-C on either CLI or **Stop** on
Android. The server can start after pairing or restart during a healthy tunnel;
new requests then work, but in-flight requests/WebSockets may fail. A tunnel
disconnect requires a fresh `share` session and offer.

## Tests

```sh
bash scripts/test-native.sh
python3 -m unittest discover -s tests/host -p '*_test.py'
node --test apps/android-controller/tests/native-stage.test.mjs apps/android-controller/tests/application-fixture.test.mjs
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
See [releasing](RELEASING.md) for signed APKs and packaged CLI binaries.
