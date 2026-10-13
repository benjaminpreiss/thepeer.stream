# Current project status

## Product and layout

Android owns a tunnel from a normal phone browser to one fixed computer-localhost
application. The route is **eligible Direct → single-carrier MASQUE Relay**.
Coordinated punching, automatic reconnect and operating our own relay are out of
scope for this MVP. Performance and discovery work are deferred.

- `crates/tunnel-core`: offers, planning, retained-peer authentication, HTTP and cleanup.
- `crates/tunnel-android`: JNI, foreground reservation and sticky Stop.
- `crates/cli` (package `thepeer-cli`): the `thepeer` desktop CLI, automatic binding and terminal QR.
- `apps/android-controller`: Android UI, scanner, service and Gradle.
- `tests/fixtures`: disposable public-data application used for host/phone checks.
- `tests/host`: terminal QR and JNI process fixtures.

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
reviewed patches 0001–0005. No Cargo-cache edits or transport upgrades.
The sole `wpa2:` codec rejects retired coordination flag 1 and bounds text to
195 bytes. Authenticate the intended computer on the retained TLS connection
before sending the scoped grant. Only classified no-response permits fallback;
authentication, protocol, input, observation, family, Stop and cleanup errors are
terminal. Browser origins use `thepeer-<32 lowercase hex digits>.localhost`;
the nonce still has 128 bits of entropy. No healthy-session lifetime or cumulative
request-count ceiling.

## Evidence and next gate

The workspace has 90 passing core tests and four helper tests. The real terminal
QR → scanner → Java/JNI → HTTP/body/CSP → Stop chain, scanner/state/handoff,
receipt and application-fixture checks pass. Android-package host compilation
also succeeds without the core `host` feature. These are host checks, not an APK build.

Before restructuring, the user rebuilt 0.6 and confirmed automatic binding,
same-Wi-Fi Direct, cellular Relay and Stop on both. Cellular Relay remains noticeably
slow; no performance diagnosis or universal NAT guarantee is claimed.

The cleaned checkout still requires operator native restaging, APK installation
and repeat physical checks after code review.

## Release automation

A manually dispatched version-tag workflow runs the existing tests, builds
macOS/Linux helpers and a signed Android APK, collects dependency notices and
checksums, and creates a draft release. The installer verifies checksums and
installs `thepeer` without sudo. Packaging/installer unit tests and workflow
linting pass locally; the renamed executable also passes the host QR/JNI flow.

The hosted release matrix, Gradle dependency-report task and signing job have not
been exercised end-to-end. Configure the protected `release` environment and
signing secrets, run the first tagged workflow, review its notices and test the
actual assets before publishing. See [release setup](RELEASING.md).

- [Release gates and proposed commit boundary](RELEASE-READINESS.md)
- [Helper build/run](HOST-HELPER.md)
- [Offer and route policy](ROUTING-OFFER.md)
- [Physical test handoff](HUMAN-TESTING.md)

Supported host entry points: `bash scripts/test-native.sh` and, with a JDK and the
pinned local ZXing jar, `bash apps/android-controller/test-native.sh`.
