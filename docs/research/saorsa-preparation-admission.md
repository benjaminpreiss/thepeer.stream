# Constructor-time helper admission

## Status and scope

The approved extension is implemented in review patch 0005 on unchanged Saorsa revision
`e2f08388088313c82cac393afa6ca27b9ce00516` (0.37.0). The constructor checkpoint passed
130 routing tests; the later [phone phase gate](saorsa-phone-ingress.md) now passes
137 routing tests and the actual helper process/terminal suite. The later
[authorization/Stop checks](saorsa-authorization-stop.md) bring routing to **144 tests**,
with no further patch change. The separate coordinated-client suite passes 91 tests. The former red diagnostic has been promoted into
three normal preparation regressions:

```sh
bash experiments/direct-traversal-review/verify-peer-preparation.sh
bash experiments/direct-traversal-review/verify-peer-routing.sh
```

The old `diagnose-preparation-admission.sh` runner and duplicate standalone probe were
replaced by the focused verifier and `native/src/peer_route_preparation_tests.rs`, which
is also included in the full routing suite. There is no skipped/expected-failure test
masking this finding.

No version upgrade, TLS/wire change, offer-format change, Android migration, production
patch promotion or public probe occurred. Existing participants need no upgrade.

## Original red and diagnosis

The bounded diagnostic observed **five simultaneous live client TLS completions while
helper-side Finished was withheld**, before `Helper::prepare` returned. A silent bound
loopback UDP participant held coordinator bootstrap pending; its first received Initial
provided the endpoint-created barrier. Five independent UDP proxies forwarded genuine
server replies but withheld client traffic after the first reply. There were no grants,
private setters, fabricated TLS outcomes or target requests.

The ready-helper control completed only four client handshakes, with zero helper
admission/authorization. Stop returned `peer-cancelled` during preparation, released the
helper bind and never accessed the target. The bound was asserted **after cleanup**.
The original output, reproduced repeatedly, was:

```text
ready_helper_control_holds_fifth_unfinished_handshake ... ok
preparation_must_not_start_a_fifth_unfinished_handshake ... FAILED
preparation bypassed admission: 5 client TLS completions while helper-side Finished was withheld
```

Ranked explanations were late registration, a second bypassing accept path, or accidental
helper-side TLS completion in the packet-loss fixture. Moving registration immediately
after endpoint construction in generated source made the diagnostic pass, supporting late
registration for the exercised bootstrap interval. The ready control checked zero helper
admission. That move also broke the old foreign-observer fixture's setup and did not place
the hook before the constructor's accept-loop startup. It was restored, not shipped as a fix.

The finding was an admission bypass, **not demonstrated exhaustion**. The original probe
ran in about six seconds after compilation and made only five attempts per case.

## Generic interface, adapter policy

The new opt-in constructor is:

```rust
P2pEndpoint::new_with_handshake_handler(config, handler)
// -> Result<(P2pEndpoint, HandshakeRegistration), EndpointError>
```

- Existing `P2pEndpoint::new(config)` and `NatTraversalEndpoint::new_with_socket(...)`
  signatures and default dispatch remain intact. No mandatory configuration field.
- The initialized registry is passed through the private constructor chain into the
  endpoint **before `spawn_accept_loop`**. This is not a post-construction setter.
  Callbacks may run before construction returns.
- The returned registration retains normal identity-safe removal/replacement semantics.
  Removal does not close connections already transferred to the caller.
- Once construction is polled, a guard detaches the initial handler on failure or
  cancellation. Successful construction transfers shutdown ownership to the endpoint.
  An unpolled future has not installed a handler and does not call `detached`.
- Accept/dial dispatch also checks the registry's stopped state. Closing a failed
  construction's registry cannot reopen default dispatch while transport shutdown has
  not yet set its separate flag. Callbacks still run outside registry locks.
- Ordinary-only handlers remain ordinary-only; virtual ingress still requires the
  explicit `incoming_virtual` override. No route, grant, quota or timeout policy moves
  into upstream code.

The helper's adapter installs an initially **not-ready** inbox through this constructor.
Before preparation completes it explicitly refuses unsolicited ordinary/virtual incoming
attempts and drops coordinator-triggered outgoing instructions without starting them.
Its explicit outgoing observer/control/carrier operations retain normal transport handling.
After successful preparation, a one-way atomic transition enables the existing shared
four-active/four-queued inbox. There is no remove/reinstall interval or default-path gap.
Only then can a TLS winner receive the unique inspector; pin/grant/application behavior
is unchanged. Incoming TLS expiry remains ten seconds, not a healthy-session lifetime.

## Observer evidence is retained, not silently deleted

The previous helper `foreign` case depended on accepting unsolicited incoming TLS during
observation. Under the new policy it must instead receive actual QUIC `CONNECTION_REFUSED`,
while the selected control completes genuine TLS but its observation is withheld. The
helper still returns no offer on observation timeout and releases its socket.

A separate public transport test in `peer_construction_tests.rs` retains the original
provenance positive control. An initially installed pass-through handler allows two real
peers. Packet loss withholds the selected peer's observation. The independent foreign
peer genuinely emits an encrypted observation, and the endpoint-wide observation becomes
the real receiver address, while the selected connection's snapshot remains `None`.
Thus global observation remains demonstrably non-substitutable for selected-peer evidence.
This is transport-interface coverage; it is not a claim that foreign TLS still reaches
helper preparation. Existing eligible/ineligible selected-observer, closure and Stop
regressions also pass.

## Verification and limits

Six added tests take the routing suite from 124 to **130**:

- Ready-helper four-active positive control.
- Five explicit preparation refusals, Stop, no offer/target and UDP release.
- Preparation task abandonment, refusals, UDP release and uncancelled parent token.
- Initial registration, real stream exchange, removal, stale-token isolation and restored
  default acceptance without closing the transferred connection.
- Bind failure detaches once; unpolled construction does not detach; successful reuse
  and repeated shutdown notify once per installation.
- The real two-peer/global-versus-selected observation distinction above.

Generated-only mutations were caught and restored: ignoring the initial registry produced
zero preparation refusals instead of five; enabling the ready phase early produced one
instead of five; disarming the construction guard before awaiting the constructor produced
zero detach notifications on bind failure instead of one.

The constructor ordering is also source-audited. Tests do **not** inject a deterministic
pause halfway through constructor polling: cancellation evidence covers unpolled
construction and abandonment during real helper bootstrap, plus the guard's failure path.
Do not describe that as a separately forced mid-constructor cancellation race.

These bounds concern opted-in **helper peer ingress**, not every transport connection.
The later [phone follow-up](saorsa-phone-ingress.md) also opts routed phone endpoints in.
Explicit outgoing controls, generic callers using the unchanged constructor, QUIC's own
pre-accept buffers, sustained-flood fairness, coordinated outgoing
contention, post-TLS authorization races and public NAT reachability remain separate
policy/verification work. `max_connections(12)` is not a proven global bound. Raw shared-
runtime UDP retention is still handled by our existing private-runtime ownership, not
claimed fixed upstream. Android remains deferred.

## Receipt and sources

Current 0005 SHA-256:
`5f4f7e02a92fd01e5bcb721958b633574e1d8da8eeeacd6475803886cd19e386`.
The prior virtual-only revision was
`4b81ea37429f83a685a593cdf469e074f2a027ad5f87cfb357278872bba01300`.
Patches 0001–0004 and the source revision are unchanged. Production still applies only
0001/0002. No Cargo-cache edits, upstream submission or acceptance are claimed.

Current sources: [adapter](../../crates/tunnel-core/src/peer_route.rs),
[preparation tests](../../crates/tunnel-core/tests/internal/peer_route_preparation_tests.rs),
[review patch](../../patches/saorsa-transport/0005-peer-handshake-hook.patch),
and the original pinned
[`P2pEndpoint`](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/p2p_endpoint.rs) /
[`NatTraversalEndpoint`](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/nat_traversal_api.rs)
constructor chain.

Historical logs: `/tmp/wpt-preparation-admission-{red,repeat,early-hook,early-observer,restored-red,restored-observer}.log`.
Current logs: `/tmp/wpt-constructor-{admission-first,preparation-green,hooks-first,observer-green,guard-mutation,install-mutation,phase-mutation,full,full-final,helper,preparation-final,coordination-first,receipt-full,receipt-helper}.log`.
