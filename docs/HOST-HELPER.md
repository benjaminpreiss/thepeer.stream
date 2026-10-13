# thepeer.stream desktop helper

`crates/cli` (package `thepeer-cli`) builds the `thepeer` executable. Shared transport, grants,
HTTP policy and cleanup live in `crates/tunnel-core`; JNI lives in
`crates/tunnel-android`. No implementation is copied between packages.

## Build and run

With Rust, the pinned dependencies and native build prerequisites already cached:

```sh
bash scripts/build-helper.sh
node tests/fixtures/application-app.mjs 3002
# In another interactive terminal:
./target/build/debug/thepeer --target-port 3002
```

The build is locked/offline and applies the exact reviewed transport patch stack.
On a new machine, dependency fetching is a separate operator step:
`cargo +stable fetch --locked` from the repository root. No helper or participant
is started by the build script. `CARGO_TARGET_DIR` can reuse an existing cache.

To prepare Relay with the previously approved participant:

```sh
./target/build/debug/thepeer --target-port 3002 --relay 207.148.94.42:10000
```

That command contacts the selected public service. It does **not** operate a relay.
Do not probe other participants without agreement. Service address and allocated
relay contact are distinct. Allocation occurs before the QR is published.

## Local-address selection

`--bind` is optional. The helper asks the OS for a source address toward the
explicit `--observer`, otherwise `--relay`. Without either it uses the numeric
IPv4 documentation address `192.0.2.1:9` as a routing-table hint. This temporary UDP
socket sends no discovery datagram; its port is discarded. The actual transport
binds the selected IP and obtains its own observation when configured.

VPNs and routing policy can select the wrong interface. Override explicitly:

```sh
./target/build/debug/thepeer --bind 192.168.2.120:0 --target-port 3002
```

No wildcard/loopback fallback or public-IP service is used. Mixed participant
families fail before discovery. Targets are fixed `127.0.0.1:PORT`, never caller
URLs or hostnames. Optional `--observer IP:PORT` supplies observation only;
`--coordinator` and coordinated punching have been removed.

## Pairing and ownership

Use the matching thepeer Android app from the same release or source revision.
The sole offer is `wpa2:`. It retains the helper's bound address, its ordinary
observation, and a separately allocated relay contact. See [wire policy](ROUTING-OFFER.md).
Old coordination-bearing offers (flag 1) are rejected, not translated.

The terminal displays a fresh capability-bearing QR, never a plaintext fallback.
Redirected output is refused. Scan only with the app's scanner, not Brave. Never
save, log, screenshot or copy the QR. `q` or Ctrl-C removes the surface and stops
the helper. A selected TLS winner consumes the one-shot offer; start a fresh helper
for another Connect. No grant file or automatic reconnect exists.

The phone authenticates the intended computer on the **retained TLS connection**
before sending the grant. Observer connections never receive it. Only classified
no-response advances routing; TLS/authentication, input, observation, family,
Stop and cleanup errors are terminal.

Unsolicited handshakes are refused during preparation. Ready helper admission is
bounded to four active and four queued attempts, with a ten-second active TLS
budget. This is not a proven global endpoint cap, sustained-load guarantee or
protection against a malicious first winner denying one-shot availability.

Healthy sessions have no lifetime or cumulative stream-count limit. Concurrent
requests, headers, bodies, directional forwarding, target operations and teardown
remain bounded. Stop/drop closes owned browser/transport resources. Relay withdrawal
is not a promise of immediate remote quota/FD reclamation (upstream ADR-011).

## Host validation

```sh
bash scripts/test-native.sh
JAVA_HOME=/path/to/jdk bash apps/android-controller/test-native.sh
```

The second command requires the checksum-pinned local ZXing 3.5.3 jar; it downloads
nothing. It exercises the real terminal QR → scanner → Java/JNI → browser HTTP/CSP
→ Stop path, plus scanner/handoff and receipt checks. All network fixtures are local.

For repeated checks, `THEPEER_PREPARED_WORKSPACE` may point to a trusted prepared tree;
tooling rejects stale workspace sources/manifests/lockfiles. Do not modify Cargo's
cached upstream checkout. Reuse a target directory and disable incremental builds
rather than accumulating duplicate build trees.

Pre-split physical checks passed Direct on Wi-Fi and Relay on cellular, including
Stop and automatic binding. Relay slowness is known but unmeasured; optimization
and discovery changes are deferred until after the MVP.
