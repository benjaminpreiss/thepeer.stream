# Current project status

## Product and layout

Android or a second desktop CLI owns a tunnel from a normal browser to one fixed
computer-localhost application. The route is **eligible Direct → single-carrier MASQUE Relay**.
Coordinated punching, automatic reconnect and operating our own relay are out of
scope for this MVP. Performance and discovery work are deferred.

- `crates/tunnel-core`: offers, planning, retained-peer authentication, HTTP and cleanup.
- `crates/tunnel-android`: JNI, foreground reservation and sticky Stop.
- `crates/cli` (package `thepeer-cli`): `share` host, terminal one-shot offer/Android QR, and `connect` desktop browser gateway.
- `apps/android-controller`: Android UI, scanner, service and Gradle.
- `tests/fixtures`: disposable public-data application used for host/phone checks.
- `tests/host`: terminal PTY/offer, HTTP restart and JNI process fixtures.

The browser controller/WASM application, relay-status site, transport experiments,
old native/Java diagnostic/viewer implementations and their obsolete guides/runners
have been removed—not kept as alternate source trees. Research findings and license
provenance remain in documentation. The unused `deployment/saorsa-relay/`
scaffold was also removed; no remote service was changed. Other deployment work
is untouched.
Ignored local build/dependency artifacts were preserved under
`target/retired-local-artifacts/`; they are not source or release inputs.

## Invariants

Saorsa remains 0.37.0 at `e2f08388088313c82cac393afa6ca27b9ce00516` with unchanged
reviewed patches 0003–0005. No Cargo-cache edits or transport upgrades.
The sole `wpa2:` codec rejects retired coordination flag 1 and bounds text to
195 bytes. Authenticate the intended computer on the retained TLS connection
before sending the scoped grant. Only classified no-response permits fallback;
authentication, protocol, input, observation, family, Stop and cleanup errors are
terminal. Browser origins use `thepeer-<32 lowercase hex digits>.localhost`;
the nonce still has 128 bits of entropy. No healthy-session lifetime or cumulative
request-count ceiling.

## Evidence and next gate

Host validation includes core, CLI, terminal PTY/HTTP restart, Android scanner/JNI,
receipt and application-fixture checks. Test counts from earlier snapshots (90/4 or
100/5) are not a current total; rerun the suites for a count at a specific commit.
These host checks do not substitute for a signed APK/device or public-network test.

Earlier Android physical checks confirmed automatic binding, same-Wi-Fi Direct,
cellular Relay and Stop on both; cellular Relay was noticeably slow (unmeasured).
Subsequently, the terminal Direct path was exercised between a macOS client and
an OrbStack Linux VM, including browsing before and after a dev-server restart;
the Android QR path was also checked. These are specific checks, not proof of
universal NAT reachability. Terminal-to-terminal Relay has not been physically
verified. Repeat physical checks on packaged binaries and a signed APK before
claiming release validation.

## Release automation

A manually dispatched version-tag workflow runs the existing tests, builds
macOS/Linux helpers and a signed Android APK, collects dependency notices and
checksums, and creates a draft release. The installer verifies checksums and
installs `thepeer` without sudo. Packaging/installer unit tests and workflow
linting pass locally; the renamed executable also passes the host QR/JNI flow.

Do not infer a hosted, published or signed release from local packaging tests.
Verify the hosted matrix, Gradle dependency-report task, signing job and real
assets in the release workflow before publishing; see [release setup](RELEASING.md). See [release setup](RELEASING.md).

- [Release gates and proposed commit boundary](RELEASE-READINESS.md)
- [CLI build/run](CLI.md)
- [Offer and route policy](ROUTING-OFFER.md)
- [Physical test handoff](HUMAN-TESTING.md)

Supported host entry points: `bash scripts/test-native.sh` and, with a JDK and the
pinned local ZXing jar, `bash apps/android-controller/test-native.sh`.
