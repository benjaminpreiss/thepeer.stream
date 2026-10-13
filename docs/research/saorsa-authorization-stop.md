# Authorization/Stop ordering: bounded real-wire checks

## Scope

Seven tests at the existing helper preparation, opaque offer, phone connection, browser
and lifecycle seams. **No production behavior fix, transport interface expansion, patch
change or Android migration.** All traffic is loopback-only, using the Direct route.

```sh
bash experiments/direct-traversal-review/verify-peer-authorization.sh
```

Source: [peer_route_authorization_tests.rs](../../crates/tunnel-core/tests/internal/peer_route_authorization_tests.rs).
The tests are also part of the full routing runner. This follows the
[phone ingress investigation](saorsa-phone-ingress.md). The current phone policy
is permanently closed; historical coordination scenarios below are not current routes.

## Actual wire barriers, not simulated authorization

A UDP proxy preserves genuine QUIC-v1 Initial/Handshake packets and holds encrypted
1-RTT tails in one direction. It never edits TLS/application plaintext, extracts the
grant, fabricates a handshake result or sets private authorization state. The helper's
actual offer is encoded/decoded in memory; the normal phone connection performs actual-
connection pin checking and grant authorization. No capability is logged or persisted.

- **Server-to-phone hold:** public helper progress reports authorized, held encrypted
  traffic exists, and the phone's connection future is still pending. The helper has
  legitimately accepted the grant, but the phone cannot receive its application ACK.
- **Phone-to-helper hold:** helper progress reports one TLS admission and no authorization,
  held encrypted traffic exists, and connection setup remains pending. The helper has not
  received client application traffic. This does not claim an exact remote parser offset
  or that a particular buffered encrypted packet contains the complete grant record.

Each direction has a positive control: releasing unchanged held bytes lets the normal
connection finish and carries actual browser HTTP 200/body/CSP through the fixed target.
This distinguishes controlled packet loss from broken crypto or unusable fixture traffic.
The proxy buffers at most 32 datagrams/128 KiB; each case has a 40-second outer budget.

## Checks

1. Release held server traffic after helper authorization; real browser HTTP succeeds.
2. Release held client application traffic; real authorization and browser HTTP succeed.
3. Stop the phone while the helper is already authorized but the ACK is withheld.
   Connection must return `peer-cancelled` before the authorization timeout, not fallback.
4. Drop the pending phone setup at that same barrier. Its UDP bind is released without
   cancelling the parent's token; late encrypted traffic cannot access the target.
5. Stop the helper before delivering held client application traffic, then release it.
   Helper completion leaves authorization false; the phone returns a terminal error,
   not `route-unavailable`. No target access.
6. Complete authorization and obtain `Connected`, then cancel its setup token before
   serving. A valid browser request is already queued on the TCP listener. Serving must
   close without contacting the target, even with an independent, uncancelled serve token.
7. Four bounded ACK-release/phone-Stop trials, alternating public-event order. Stop-first
   must cancel setup. Release-first may return either cancellation or a completed session;
   any returned session must still reject queued browser work because its setup token is
   cancelled. Both binds are reusable and no target connection survives.

The race trials do not force an internal scheduler interleaving, require both possible
winners to occur, or prove fairness. The separately forced post-success/pre-serve case
covers a completed session even if all race trials cancel before return. Authorization
may already be true before phone Stop; Stop is not claimed to roll back an accepted grant.

## Mutation sensitivity

Generated-only mutations, restored afterward:

| Mutation | Observed failure |
| --- | --- |
| Ignore phone setup cancellation | Held-ACK Stop exceeds the eight-second test budget, before the ten-second auth deadline |
| Ignore the setup token in `Connected::serve` | The queued browser request actually contacts the target after Stop |
| Ignore helper task cancellation | Helper completion exceeds the bounded wait after held client traffic is released |

No mutation changes a test's TLS result or routing decision. Both packet-hold directions
have real application positive controls. Root production behavior remains unchanged.

## Verification and remaining work

This checkpoint passed **144 tests** (137 plus seven authorization/Stop checks) and the
real helper process/terminal suite. The later [paced contention case](saorsa-paced-contention.md)
passes 145 routing tests and the helper suite (after approved incremental-cache cleanup).
The separate coordinated-client suite's
last milestone remains 91; it was not rerun for this routing-only test addition.

This is not exhaustive race coverage, a coordinated/relayed authorization cut-point test,
malformed/replayed-frame coverage, sustained contention, CPU/RSS measurement or public
NAT proof. Remaining work includes coordinated outgoing contention and longer, explicitly
bounded mixed-ingress load. Finite event-order trials do not establish flood resistance.
Android and cellular → Wi-Fi verification remain deferred.

Logs: `/tmp/wpt-auth-stop-{first,green,positive-controls,phone-mutation,session-mutation,helper-mutation,full,helper,full-final,helper-final,focused}.log`.
Patch 0005 remains
`5f4f7e02a92fd01e5bcb721958b633574e1d8da8eeeacd6475803886cd19e386`;
source revision and patches 0001–0004 are unchanged.
