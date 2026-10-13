# Saorsa patches required by the current Direct/Relay scope

## Initial recommendation

For the current architecture, keep **0003, 0004 and 0005**. Remove **0001 and
0002** in a separately reviewed build-input change. They are retained custom-relay
extension APIs, not prerequisites for the Relay route used by this application.

This audit examines application commit `d48a5af` against Saorsa 0.37.0, revision
`e2f08388088313c82cac393afa6ca27b9ce00516`. During the initial audit, the shipped patch files, hashes,
preparation recipe, dependencies and Cargo source cache were not modified.
An isolated generated workspace was used for the reduced-stack experiment.

"Required" here means required to retain the current routing, authentication,
admission and cancellation guarantees with this pinned upstream version—not that
all conceivable tunnels need these patches. Hostname resolution, HTTP adaptation,
CSP handling and possible browser-facing HTTPS are application-layer concerns;
they do not create a dependency on the retired relay extensions.

## 0001 — remove: custom relay forwarding policy

[Patch 0001 at the audited commit](https://github.com/benjaminpreiss/thepeer.stream/blob/d48a5afbc7173ea09f48a8a4a03861344db3bbf9/patches/saorsa-transport/0001-stream-forwarding-policy.patch)
adds `StreamForwardPolicy`, pacing callbacks, configurable forwarding limits,
`run_stream_forwarding_with_policy` and the `new_local` allocator.

No current application or Rust test calls those APIs. Production endpoint setup
explicitly disables relay serving and UPnP in
[`EndpointOwner::create_with_handler`](../../crates/tunnel-core/src/peer_route.rs).
The product consumes an explicitly configured relay; it does not operate the
custom relay service for which these policy hooks were introduced.

The patch also refactors the upstream default forwarding path to use an internal
wrapper. The no-policy/default-limit behavior remains available in unpatched
upstream. Local relay fixtures exercise that upstream behavior; removing the patch
does not remove the relay implementation itself.

This patch should be reconsidered if operating a policy-controlled relay becomes
an implemented feature. The roadmap alone is not a current runtime requirement.

## 0002 — remove: custom established-stream socket API

[Patch 0002 at the audited commit](https://github.com/benjaminpreiss/thepeer.stream/blob/d48a5afbc7173ea09f48a8a4a03861344db3bbf9/patches/saorsa-transport/0002-bounded-stream-socket.patch)
adds `MasqueRelaySocket::from_established_streams`, configurable queue/frame limits
and public visibility for `RelayTunnelControl` and its lifecycle methods.
No current application or Rust test uses that added constructor or control API.

The CLI calls upstream `P2pEndpoint::prepare_proactive_relay`. Upstream's
`NatTraversalEndpoint::prepare_proactive_relay` creates the tunnel with its own
`MasqueRelaySocket::new`, not our added constructor. That existing constructor,
bounded queues/frame handling, and internal shutdown controls all predate 0002.

Therefore removing this patch removes unused customization/public access—not the
current Relay socket, its upstream bounds or its cleanup implementation. Conversely,
we should not claim that the current application uses the patch's custom limits:
it already uses upstream defaults with the full five-patch stack.

## 0003 — keep: bound untrusted relay control input

[Patch 0003](../../patches/saorsa-transport/0003-client-relay-response-bound.patch)
rejects zero-length or greater-than-4096-byte CONNECT-UDP replies before allocating
the reply buffer.

The active call path is:

```text
CLI prepares configured Relay before publishing QR
  → P2pEndpoint::prepare_proactive_relay
  → NatTraversalEndpoint::prepare_proactive_relay
  → establish_owned_relay_session
  → read relay-controlled length
  → validate bound, then allocate/read response
```

Without the patch, a four-byte length controls `vec![0u8; resp_len]` without this
size check. A timeout cannot bound that allocation. A malicious or faulty relay
could request an allocation approaching 4 GiB. Inner peer authentication does not
protect this earlier relay-control exchange. This path runs when Relay is prepared,
even if Direct subsequently wins.

This is a security/resource-limit patch rather than a requirement for a successful
exchange with a well-behaved relay. Keep it. The current repository has Relay
integration coverage, but this audit found no dedicated live oversized-reply
regression in the current Rust tests. Restoring that specific negative test is a
useful follow-up; a passing happy-path suite is not proof of this bound.

## 0004 — keep: owned dial and atomic no-response classification

[`peer_connect::attempt`](../../crates/tunnel-core/src/peer_connect.rs) actively
calls the [patch's](../../patches/saorsa-transport/0004-cancellable-peer-attempt.patch)
`P2pEndpoint::begin_connect` and `Connecting::abort`.

- `begin_connect` returns the actual application-owned attempt without upstream
  automatic fallback or competing application stream readers.
- `abort` inspects response/error state and cancels under the same lock. That
  prevents a deadline race from classifying a received response or recorded error
  as silence and incorrectly trying another route.
- The completed connection is retained for pin checking and grant authorization.

The upstream `first_peer_response` method already exists without this patch; the
patch adds the caller-owned entry point and atomic abort, not that observation API.

[Current tests](../../crates/tunnel-core/tests/internal/peer_connect_tests.rs)
exercise silent fallback, stalled TLS, wrong pins, forged signatures, atomic
abort classification, Stop and preservation of unrelated shared connections.

One smaller portion is test-only in this repository: making
`accept_with_connection` public. Production CLI admission now obtains accepted
handles through 0005. Several fixtures still need public access to the exact
accepted connection. It could be reduced only alongside fixture changes; that is
not a reason to remove the whole patch.

## 0005 — keep the admission hooks; coordination request API is unused

[Patch 0005](../../patches/saorsa-transport/0005-peer-handshake-hook.patch)
provides constructor-installed handshake ownership. Its active consumers are
[`PhoneIngress`](../../crates/tunnel-core/src/phone_ingress.rs) and the CLI's
[`Inbox` / `EndpointOwner`](../../crates/tunnel-core/src/peer_route.rs).

- The phone refuses unsolicited ordinary and virtual handshakes, including during
  observer setup and endpoint construction.
- The CLI refuses ingress during preparation, then admits bounded pending work
  with a TLS deadline and a single winning connection.
- Virtual ingress hooks apply those rules to the relay socket, not just Direct.
- Both roles reject coordinator-requested outgoing handshakes.

**The outgoing hook is still needed even though coordinated punching is out of
scope:** it blocks upstream from carrying out unwanted coordinator instructions.
Removing a feature from route selection is not the same as disabling all upstream
entry points that can trigger it.

Preparation, ingress, admission, contention and teardown are exercised by the
[current routing test modules](../../crates/tunnel-core/tests/internal/routing.rs).

There is removable API surface within this patch:

- `request_coordinated_peer` has no current application or test callers and serves
  an out-of-scope capability. It is the clearest candidate for a later trim.
- Public post-construction handler installation/removal has no current application
  or test callers. Constructor installation still needs internal hook machinery;
  removing the public methods must not remove shutdown or failed-construction guards.
- `PeerDial::connect` and `remote_address` are not used by our handlers, which reject
  the dial. The outgoing interception itself must remain.

For the lowest-risk first reduction, keep 0005 unchanged. A smaller handshake-only
patch can be reviewed separately rather than combining API surgery with removal
of the two unrelated relay-extension patches.

## Reduced-stack experiment

The normal recipe generated a disposable source/workspace under
`target/patch-audit/run.XERQJY`. In that generated Saorsa source only, 0002 and 0001
were reverse-applied. Both reversals passed `git apply --check`.

The resulting `src/masque/relay_server.rs` and `src/masque/relay_socket.rs` match
the exact upstream Git blobs byte-for-byte. Patches 0003–0005 remain applied.
The application's source was unchanged for the experiment.

Validation with Rust 1.95.0 and locked/offline dependencies passed:

- Full workspace tests, two test threads: **100 core tests and 5 CLI tests**, plus
  documentation tests. Core tests completed in 187.11 seconds.
- Separate `cargo check -p tunnel-android`: passed without enabling the CLI's
  host feature through workspace feature unification.

Commands (the paths below identify only the disposable audit workspace):

```sh
cargo +1.95.0 test --locked --offline --manifest-path target/patch-audit/run.XERQJY/tree/Cargo.toml --workspace -- --test-threads=2
cargo +1.95.0 check --locked --offline --manifest-path target/patch-audit/run.XERQJY/tree/Cargo.toml -p tunnel-android
```

This is a local host experiment, not Android cross-compilation, QR/JNI integration,
physical-device validation, independent security review or evidence about every
upstream transport path. The host check of the Android crate is not an APK build.

## Approved implementation follow-up

The supported recipe now applies only 0003–0005. The unused 0001/0002 files and
hash-list entries are removed; the upstream revision, dependencies, remaining
patch bytes and upstream notices are unchanged. Node and Gradle receipt membership
and their [tests](../../apps/android-controller/tests/native-stage.test.mjs) agree
on the three-patch stack. Older Android source receipts require rebuilding.

Prepared workspaces now record recipe and patch hashes in `preparation.json`.
The reuse script rejects missing, malformed or mismatching input receipts, so
old prepared trees cannot silently keep the five-patch stack. This assumes a
trusted generated tree; it is not verification against manual source tampering.

A fresh official preparation passed 100 core and 5 CLI tests, the host QR/JNI
integration with IPv4-only and IPv6-only targets, 8 Node checks and 19 Python
tests. The user also reported that the isolated reduced-stack CLI worked with the
existing app. Android cross-build and reduced-stack APK/device validation remain
operator gates; do not reuse an Android artifact receipt from the five-patch build. Current build policy is documented in
the [patch README](../../patches/saorsa-transport/README.md),
[architecture](../ARCHITECTURE.md#why-we-carry-three-patches) and
[divergence overview](saorsa-upstream-divergence.md).
