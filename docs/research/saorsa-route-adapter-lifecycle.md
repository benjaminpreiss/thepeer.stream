# Route adapter: first direct/lifecycle slice

Date: 2026-10-11. Application adapter work only; no new or modified upstream patches.

Retained for its diagnosis of endpoint/runtime ownership. The direct-only scope,
one-handshake helper and 90-test count below describe the original investigation,
not the current bounded Direct/Relay helper. See [current routing](../ROUTING-OFFER.md).
The later [active-stream correction](saorsa-route-stream-teardown.md) makes runtime
Drop drain endpoint supervisors before termination and supersedes the original
eight-second runtime-owner completion budget. Unpolled-drop success alone had not
established correct active-stream teardown.

## Agreed interface and implemented scope

The user approved testing through helper preparation, phone connection and session
Stop/drop, rather than mocking internal route decisions. Implementation is in
`apps/android-controller/native/src/peer_route.rs`, behind the separate host-only
`routing-client` feature (which depends on the existing review interfaces).

- `Helper::prepare(config, stop)` owns a fixed localhost target, fresh computer
  identity and fresh one-use grant, and returns an opaque in-memory `Offer`.
- `connect(offer, bind, stop)` consumes the offer and retains the exact checked
  application session and its transport ownership. The grant is zeroizing memory;
  the offer has no Debug/Clone/serialization implementation.
- `Connected::serve(listener, hostname, stop)` reuses the actual application gateway.
  Both the setup cancellation token and the supplied serving token can stop it.
- `Helper::shutdown` cancels only its child token and waits for owned cleanup.
  Drop signals teardown without needing to spawn work from a destructor.

**Only the direct route is wired in this first slice.** It uses the existing
phase-aware `peer_connect::open` and actual pin-before-grant authorization. Direct
unavailability currently ends with `route-unavailable`; every other error remains
terminal. There is no automatic reconnect or healthy-session lifetime limit.

The helper currently accepts one handshake, closes its inbox and transfers its
single inspector into application serving. This is deliberately an incomplete
foundation, **not** the final overlapping-handshake helper: coordinated and relay
branches, the asymmetric-loss regression, bounded overlapping helper handshakes and
route-winner ownership still need implementation. No claim of route ordering follows
from the direct-only test.

Config uses an explicit bind address, fixed nonzero localhost target port, disabled
relay serving/UPnP/external advertisement, and a concurrent connection bound. There
is no special LAN transport mode. No public participant is contacted by this slice.
No new QR encoding, Android integration, SDK build, staging/install or release.

## Cleanup issue found by the tests

The first real HTTP test went red with `route-not-implemented`, then green. Two
additional public-interface lifecycle tests exposed a genuine missing guarantee:
awaiting endpoint shutdown did not release the UDP bind, including after dropping
an unpolled browser future. The latter remained bound for the 15-second fixture budget.

A minimized diagnostic removed the adapter, application connections and generic
handshake hook entirely. On the existing **four-patch** prepared stack:

1. Construct a loopback `P2pEndpoint` with serving/advertisement/UPnP disabled.
2. Await `shutdown`, then drop the endpoint.
3. Try rebinding its UDP address for five seconds on the still-running Tokio runtime.

This reproduced `AddrInUse`/deadline expiry. It does not establish which internal
reference retains the socket, or prove it remains bound forever. It does show that
successful public `shutdown` completion alone is not the required socket-release
acknowledgement. Pinned-source shutdown drains connections and signals tasks; it does
not expose an independently awaited P2p endpoint/socket destruction operation.

A comparison ending a dedicated Tokio runtime released the same kind of socket.
The existing JNI viewer runner already owns a current-thread runtime per native run;
this is useful precedent, not evidence that the new adapter is Android-integrated.

### Adapter-only resolution

`native/src/route_runtime.rs` gives each prepared helper/connected phone transport
an application-owned current-thread runtime on a private OS thread. Transport setup
and its background tasks run there. The wrapper retains runtime ownership alongside
the session; browser gateway work can use the caller's runtime.

Explicit completion first closes peer/application work and requests graceful endpoint
shutdown, then ends the private runtime and awaits the worker's completion signal.
Runtime shutdown has a bounded wait for blocking tasks; that budget applies only to
teardown, never to a healthy session. Dropping an owner closes its stop channel so the
already-running thread can clean up even without spawning from the caller's destructor.
Abandoning setup also stops its private runtime. This avoids letting retained transport
tasks outlive the application owner on a shared caller runtime.

The raw shared-runtime diagnostic remains red; we have not fixed or patched Saorsa's
shutdown implementation. The adapter's two socket-release regressions are now green.
Further active-stream/partially-polled cancellation and fallback cleanup tests remain
necessary; these results must not be generalized to every cancellation interleaving.

## Evidence and reproduction

```sh
# Fresh preparation, unchanged hash-verified patches 0001–0005, locked/offline,
# synthetic loopback only. All arguments (including --public) are rejected.
bash experiments/direct-traversal-review/verify-peer-routing.sh
```

**90 tests passed**: the existing 87 native/generic-hook tests plus:

1. Prepare → authorized direct connection → real browser HTTP 200, CSP and fixture
   body → Stop → both owners finish and the browser listener is closed.
2. Helper shutdown releases its UDP socket, leaves the parent token uncancelled and
   never contacts the target.
3. Dropping an unpolled browser future releases the phone UDP socket/browser listener
   without target access or parent-token cancellation.

The production-source host JNI ABI/lifecycle check passed again; not an Android test.
Rust formatting, shell syntax and public-option rejection passed. The existing
coordination runner and CI feature selection remain unchanged. The new routing runner
is separate while this module is incomplete.

Logs: `/tmp/wpt-route-direct-red.log`, `/tmp/wpt-route-direct-green.log`,
`/tmp/wpt-route-lifecycle.log` (two lifecycle failures),
`/tmp/wpt-route-cleanup-repeat.log`, `/tmp/wpt-route-raw-shutdown.log`,
`/tmp/wpt-route-runtime-comparison.log`, `/tmp/wpt-route-runtime-green.log`,
`/tmp/wpt-route-full.log` (90), `/tmp/wpt-route-production-jni.log`.

The standalone minimized diagnostic and experiment runners have been removed.
Current lifecycle regressions are in
[core lifecycle tests](../../crates/tunnel-core/tests/internal/lifecycle_tests.rs)
and [stream teardown tests](../../crates/tunnel-core/tests/internal/peer_route_stream_tests.rs).
Use [current build/test instructions](../BUILDING.md). Coordinated fallback is not
part of the current Direct/Relay implementation.
