# Direct/Relay MVP release review

## Current scope

The current implementation has host validation, but this is not a completed
independent security review or validated binary release. Review all intended source,
tests, build tooling and notices together, including newly added files.

The supported design is eligible Direct → prepared single-carrier MASQUE Relay.
No coordinated punching, legacy viewer/relay offers, browser-owned tunnel, grant
files or automatic reconnect belong to this MVP. Saorsa stays at 0.37.0, revision
`e2f08388088313c82cac393afa6ca27b9ce00516`, with reviewed patches 0003–0005.

## Cleanup/package split

- Root Rust workspace: `crates/{tunnel-core,tunnel-android,cli}`.
- Core owns offers/planning, retained-connection pin/grant authorization, HTTP and
  owned transport teardown. Its larger tests live in `tests/internal/`, attached
  to private modules without widening the production API.
- Android owns JNI, foreground reservation, synchronized token installation and
  sticky Stop. Existing JNI library/symbol names are preserved.
- `crates/cli` (package `thepeer-cli`, binary `thepeer`) owns `share` text/QR UI,
  `connect` hidden-prompt pairing and loopback-only desktop browser gateway,
  argument parsing and automatic bind selection. Build overrides use `THEPEER_*`.
- Per-session browser origins use `thepeer-<32 lowercase hex digits>.localhost`;
  generators, exact validation and fixtures are updated together.
- Constructor-installed phone ingress is always closed to unsolicited ordinary,
  virtual and coordinator-requested handshakes. Helper admission budgets remain.
- Removed the native coordinated adapter, old diagnostic/viewer/paired-relay and
  relay-policy/bandwidth implementations. Live HTTP framing, one-shot authorization
  and progress were extracted first; production no longer imports experiment code.
- Removed the inactive Android controller service, bundled diagnostic assets,
  socket probes and their obsolete runners/tests. Only the current connection UI,
  scanner, service and JNI bridge remain in the app source set.
- Removed coordination records from the offer. Bit 1 is rejected; supported `wpa2:`
  layouts are 73–142 decoded bytes / 103–195 text bytes. No compatibility shim.
- Android receipts now enumerate workspace manifests/lock and core/Android production
  sources, not desktop Rust or separate test files. Strict source/artifact checking
  remains. Core source, manifests, lock or patches changing still require restaging.
- CI now runs the supported workspace, scanner, receipt and helper→JNI suites—not
  retired relay/coordinator/browser-WASM jobs. A separate manually triggered
  version-tag workflow builds assets and creates a draft release. Publication
  requires operator review; no relay deployment is performed.

The obsolete browser/WASM application, relay-status site and experiment source
implementations have now been deleted, not kept as archived alternate builds. Live
application fixtures moved to `tests/fixtures/`; obsolete guides and contradictory
status/vision history were removed or replaced. Dated research findings and license
provenance remain. The unused `deployment/saorsa-relay/` Ansible scaffold was
removed with user approval; no remote service was changed. Other deployment work
is untouched. Ignored local
build/dependency artifacts were moved to `target/retired-local-artifacts/`, not deleted.

## Evidence and remaining gates

See [validation history](VALIDATION.md) for build-specific checks and the
[physical testing protocol](HUMAN-TESTING.md) for repeatable device checks.
Host fixtures do not validate packaged binaries or the signed APK; test counts
must be obtained from a run at the release commit. Do not bypass native receipt
verification.

Before release:

- Review the implementation and deletions, including newly added files.
- Verify the hosted [draft-release workflow](RELEASING.md), matrix builds, Gradle
  dependency collection and signing on real assets. Do not claim a published
  release from local tests or assume a workflow run has occurred.
- Test matching packaged CLIs on each supported platform and the signed APK on a
  phone, not just the source-build fixtures. Test Direct `share` / `connect` and
  restart behavior; test Relay explicitly with an authorized participant.
- Complete the operator build/device checks above.
- Review actual distributed APK/helper dependencies and include required license/
  copyright/NOTICE texts. Cargo metadata alone is not a binary notice bundle;
  Android/Gradle dependencies and bundled native notices must be considered.
- Review the distribution for private paths and unintended files. Never distribute
  live grants, QR captures, caches, test outputs or local credential material.

Known limits: cellular Relay is noticeably slow but unmeasured. No universal NAT,
NAT64, sustained adversarial fairness/CPU/RSS or global connection-cap claim is made.
A malicious first TLS winner can deny one-shot availability. Relay withdrawal does
not imply immediate remote reservation/FD reclamation (ADR-011). Performance,
discovery and additional routing strategies remain post-MVP work.

## Licensing

The project uses the MIT license. The planner and transport patches retain their
upstream attribution and verbatim MIT notices; Gradle retains its Apache-2.0 terms.
Dependency licenses remain applicable regardless of Git history. See
[third-party licensing](../LICENSING.md).
