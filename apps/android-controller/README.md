# thepeer Android app

Source version: **0.6-direct-relay-preview**, version code 6. The user confirmed
Wi-Fi Direct, cellular Relay and Stop on both before the Rust package split.
The cleaned workspace passes host checks, but requires a fresh operator native
build/APK installation and repeat device validation before release.

## Connection flow

1. Tap **Connect** for the in-app camera scanner. After granting camera permission,
   tap Connect again; permission callbacks do not open the camera.
2. Scan the trusted helper's current `wpa2:` QR. There is no legacy translation.
3. If offered, obtain a phone observation from the explicit participant, then try
   eligible Direct candidates followed by single-carrier Relay. No punching.
4. Authenticate the intended computer on the retained TLS connection, authorize
   the scoped grant, and only then open the fresh `.localhost` browser origin.
5. **Stop**, including notification Stop, closes access. No automatic reconnect.

Only conservative no-response permits fallback. Authentication/protocol, input,
observation/bootstrap, family, cancellation and cleanup failures are terminal.
Shared public IP is a LAN hint, not proof. No implicit dual-stack/NAT64 switch.

Browser launch is attempted once while foregrounded. **Open app** can reopen the
same local URL; it does not contain a grant. Service Intents carry only a generation
number. Invitations stay in memory, transfer once and are cleared on consumption
or cancellation. No clipboard, preferences, saved-state or capability logging.

Scanner backgrounding/cancellation and its 60-second expiry concern scanning,
not an established session. There is no healthy-session lifetime or cumulative
request-count limit; setup, HTTP/resource and teardown budgets remain.

## Operator build

Outside the agent sandbox, from the repository root:

```sh
bash apps/android-controller/native-build.sh
```

This builds `crates/tunnel-android` from the shared Rust workspace for arm64/API 26,
using the exact Saorsa revision plus reviewed patches 0003–0005. The JNI library
name remains `libcontroller_saorsa_probe.so`; the routing marker and receipt remain
`web_p2p_routing_client_v2` / `saorsa-routing-v2`. The old native target directory
is retained only as an NDK cache, not as a source package.

Receipts cover workspace manifests/lock, core and Android production Rust sources,
Java handoff sources, patches and native tooling. Core test files and desktop-helper
Rust files are no longer APK inputs. Workspace manifest/lock changes still require
restaging. Verification is strict; do not bypass stale-source errors.

Then open this directory in Android Studio, Sync, and perform a full **Run app**
to an arm64 device—not Apply Changes. SDK/NDK builds, installation and device tests
remain operator actions. See [physical handoff](../../docs/HUMAN-TESTING.md).

## Host checks

With an installed JDK and checksum-pinned local ZXing core 3.5.3 jar:

```sh
JAVA_HOME='/Applications/Android Studio.app/Contents/jbr/Contents/Home' \
  bash apps/android-controller/test-native.sh
bash scripts/test-native.sh
```

Run these from the repository root. They cover scanner/state/handoff, the actual
helper terminal QR → scanner → JNI → HTTP/CSP → Stop chain, native receipts,
application fixtures and shared Rust tests. No Android compilation, physical-camera
validation or public participant probing is implied.

The old diagnostic/viewer implementations, guides and runners have been removed,
as have the browser controller/WASM and relay-service experiments. The disposable
application fixture is now under `tests/fixtures/`. Use only the commands above;
nothing here deploys a public relay.
