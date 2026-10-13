# Direct/Relay MVP release review

## Current scope

The current implementation has host validation, but this is not a completed
independent security review or validated binary release. Review all intended source,
tests, build tooling and notices together, including newly added files.

The supported design is eligible Direct → prepared single-carrier MASQUE Relay.
No coordinated punching, legacy viewer/relay offers, browser-owned tunnel, grant
files or automatic reconnect belong to this MVP. Saorsa stays at 0.37.0, revision
`e2f08388088313c82cac393afa6ca27b9ce00516`, with the same five reviewed patches.

## Cleanup/package split

- Root Rust workspace: `crates/{tunnel-core,tunnel-android,cli}`.
- Core owns offers/planning, retained-connection pin/grant authorization, HTTP and
  owned transport teardown. Its larger tests live in `tests/internal/`, attached
  to private modules without widening the production API.
- Android owns JNI, foreground reservation, synchronized token installation and
  sticky Stop. Existing JNI library/symbol names are preserved.
- `crates/cli` (package `thepeer-cli`, binary `thepeer`) owns terminal/QR UI,
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

Host evidence after restructuring:

- 90 core Rust tests and four desktop bind/argument tests.
- Real helper terminal QR → scanner → Java/JNI → browser HTTP/body/CSP → Stop;
  preparation cancellation, automatic bind and socket release included.
- Scanner, 34 presentation and 23 handoff checks; five native receipt checks
  (including Node/Gradle input-list parity), two application-fixture checks and a
  repository-layout check that requires obsolete source trees to be absent.
- Separate Android-package host compilation without the core `host` feature.

The previous 157-test milestone included retired designs; it is not the current
count. Retained coverage includes actual forged-signature rejection with a positive
control, pin/grant rejection, authorization/Stop races, ingress/admission recovery,
observer provenance/failure, Direct/Relay HTTP, SSE/WebSocket teardown, and bounded
resource release. Raw test-only QUIC policy fixtures are not labelled MASQUE tests.
Validation logs are local artifacts, not release inputs.

The user previously confirmed Wi-Fi Direct, cellular Relay and Stop on both with
0.6 and automatic binding. **The cleaned checkout still needs an operator native
build, APK installation and repeat physical checks.** See [handoff](HUMAN-TESTING.md).
Existing generated JNI artifacts are stale; do not bypass receipt verification.

Before release:

- Review the implementation and deletions, including newly added files.
- Configure the `release` environment/signing secrets and complete the first
  hosted [draft-release workflow](RELEASING.md) run. The matrix builds, Gradle
  dependency collection and signing have not yet been validated end-to-end.
- Test the packaged helpers on each supported platform and the signed APK on a
  phone, not just the source-build fixtures.
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

## Fresh initial commit — review before staging

The intended publication is a new repository with **one initial commit containing
the reviewed current source, tests, documentation and licenses**, without importing
the previous Git history. Keep the old Git metadata as a backup outside the
project before initializing the new repository, or export selected source files
to a fresh directory. Do not import old Git metadata, ignored build caches,
local credentials or retired artifacts. Review these groups together:

1. **Complete MVP implementation and obsolete-path removal:** root Cargo files;
   all three crates and tests; exact transport patches/preparation; host build/test
   scripts and PTY/QR fixtures; Android activity/service/scanner/state/handoff,
   manifests/configuration and Java tests; native build/staging/receipt tooling;
   public application fixtures; supported CI and obsolete workflow removal;
   relevant ignore rules. Include the root MIT license, planner MIT notice and
   third-party licensing documentation with the implementation. Removed files
   must stay absent from the new initial commit.
2. **Guides and evidence:** README, helper/offer/Android/human-testing/release guides,
   current status and retained technical research. Keep Andrew
   Thomas attribution and distinguish pre-split device evidence from host checks.

After code review, derive explicit file allowlists, including every required
untracked file, and validate the selected tree before its initial commit. Creating
or publishing the new repository remains an operator action; no history rewrite,
staging, commit or push has been performed by this cleanup.

## Licensing

The project uses the MIT license. The planner and transport patches retain their
upstream attribution and verbatim MIT notices; Gradle retains its Apache-2.0 terms.
Dependency licenses remain applicable regardless of Git history. See
[third-party licensing](../LICENSING.md).
