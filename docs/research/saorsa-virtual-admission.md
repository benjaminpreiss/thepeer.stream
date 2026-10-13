# Opt-in virtual handshake admission

## Scope and decision

Pinned transport remains `e2f08388088313c82cac393afa6ca27b9ce00516` (0.37.0).
The user approved extending the existing generic hook, with routing, quotas and
lifecycle policy retained in our adapter. Android migration and public testing remain
deferred. No dependency upgrade, new wire format or production patch promotion.

The original [virtual accept loop](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/nat_traversal_api.rs#L6320)
spawned a task for each incoming handshake, publishing only completed TLS connections.
`accept_with_connection()` therefore could not enforce our admission limits before TLS.
The new real loopback regression was red: a fifth attempt received eight response
datagrams when it should have waited without starting TLS.

## Small generic interface; adapter-owned policy

Review patch `0005-peer-handshake-hook.patch` now adds
`PeerHandshakeHandler::incoming_virtual(Incoming) -> Option<Incoming>`:

- Default returns the original incoming to upstream handling. It does **not** invoke
  the ordinary `incoming` callback. Existing coordination handlers remain ordinary-only.
- The proactive virtual accept loop invokes this callback before starting TLS.
- A handler may own/refuse the incoming or return it unchanged, just like the existing
  ordinary callback. No grant, route selection, queue size or timeout policy is upstream.
- Callbacks run outside registry locks. Accept loops snapshot the handler, then check
  shutdown. Shutdown sets its flag before clearing hooks so registry clearing cannot
  accidentally reopen default dispatch during teardown. Previously transferred work
  remains owned by its recipient.

The helper opts in, routing virtual incoming attempts into the same inbox as its
ordinary peer attempts: **four active TLS attempts, four queued, overflow refused**.
Each active TLS attempt has the existing ten-second deadline. Queued time is distinct
from active TLS time. Expired/failed admission does not consume the unique inspector.
The selected actual connection still determines virtual provenance; an ordinary winner
withdraws unused relay allocation, while a virtual winner requires a live allocation.
The old second, post-TLS `relay_peer()` selection path is removed.

Registration now precedes proactive relay allocation, hence precedes creation of its
virtual listener and any early virtual ingress. Ordinary coordinator observation remains
transport-owned before registration. This is **not** a claim that ordinary ingress from
initial endpoint construction is covered by the hook: that earlier construction/control
window was confirmed by the later preparation diagnostic. The approved
[constructor-time extension](saorsa-preparation-admission.md) now closes that helper
window and promotes the diagnostic into the green suite (130 tests).

## Real endpoint evidence

`peer_route_admission_tests.rs` tests the approved helper/offer/connection/Stop seams:

1. Real loopback relay allocation; four clients start TLS through separate UDP proxies.
   Returning server flights are lost, so TLS remains unfinished. Four more attempts wait
   without replies. The ninth receives actual QUIC `CONNECTION_REFUSED`. No grant is given
   to these clients, no helper admission/authorization or target TCP access occurs, and
   Stop releases the helper UDP bind while the participant remains running.
2. Four unfinished attempts lose both directions after the first server reply, preventing
   old Initial retransmissions from entering admission again after expiry. After eleven
   seconds, one legitimate phone connection uses a decoded offer. Direct packet silence
   leads to Relay, where real browser HTTP returns 200, body `recovered`, and CSP. Stop
   closes the browser listener and ends owned work. No automatic reconnect is used.

The generic hook fixture additionally proves that an ordinary-only receiver does not
capture virtual ingress. An explicitly opted-in receiver owns the actual virtual
connection, exchanges real stream data, and can remove its registration without closing
that already-transferred connection.

Generated-only sensitivity checks, restored afterwards:
- Lengthening active TLS admission to thirty seconds makes the browser recovery fail
  with `route-unavailable`.
- Dispatching virtual ingress into the ordinary callback makes the isolation fixture
  fail its bounded completion check.

At the initial extension checkpoint, `verify-peer-routing.sh`: **122 passed** (119 plus
two admission regressions and one generic hook regression). Fresh `verify-helper.sh` also passes actual terminal/QR,
HTTP, session cleanup, Stop, argument and output-refusal checks. No public traffic,
Android build/staging/install or deployment occurred.

## Patch receipt and limitations

Current 0005 SHA-256 (including constructor-time installation):
`5f4f7e02a92fd01e5bcb721958b633574e1d8da8eeeacd6475803886cd19e386`.
The virtual-only revision documented in this slice was
`4b81ea37429f83a685a593cdf469e074f2a027ad5f87cfb357278872bba01300`.
Previous ordinary-only 0005 SHA-256:
`555aa4a89e02131a62e2596cf50a641e74d10332d70007c8f74f66ce4e4b1e3c`.
The preparation verifier checks the new hash. Patches 0001–0004 and the pinned revision
are unchanged; production preparation still applies only 0001/0002. No upstream
submission or acceptance is claimed. Loopback relay fixture substitutions are excluded
from the generated patch.

These are bounds on **hook-owned** work, not a twelve-connection transport guarantee.
`max_connections(12)` is still only configured intent on this path. QUIC's own
[`max_incoming`](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/config/mod.rs#L460-L474)
limits pre-accept objects separately; its buffering and ingress for non-opted-in
callers are not reduced to four by these changes. Nor do these tests prove fairness under sustained
flooding, coordinated outgoing dials under contention, post-TLS authorization/Stop races,
public NAT reachability, or remote reservation/FD reclamation. The bounded mixed-ingress
follow-up below is finite recovery evidence, not a sustained-flood availability claim. A completed unauthorized TLS winner
still ends the one-shot helper according to its existing policy; this does not promise
availability against an attacker who wins TLS first.

Logs: `/tmp/wpt-virtual-admission-{red,green,opt-in,expiry,timeout-mutation,isolation-mutation,full,helper,full-final,helper-final}.log`.

## Follow-up: repeated mixed-ingress waves and drop

Two further regressions use the same public helper, opaque offer, phone connection,
real browser and Stop/drop seams. No implementation, interface, patch, dependency,
Android or production-preparation changes were needed.

Each wave alternates ordinary UDP and virtual ingress into **one** four-active/four-queued
budget. Two further attempts, one per ingress kind, must receive actual QUIC
`CONNECTION_REFUSED`. The next wave reverses which ingress arrives first. UDP proxies
forward server replies but suppress client traffic after the first reply, including
Finished and Initial retransmissions. Thus client TLS may complete while helper-side TLS
stays unfinished, without allowing expired attempts to re-enter admission.

The fixture waits for the queued batch to respond after active expiry, then requires all
four queued clients to complete **real client-side TLS**. A late refusal is not accepted
as proof of promotion. Their helper-side attempts still expire; no grant is supplied to
any overload connection and helper admission/authorization remain zero.

- **Drop:** two complete expiry cycles on the same helper, then a third saturated wave
  (thirty attempts total, including six overflow probes). Dropping that helper releases
  its UDP bind within eighteen seconds, leaves its parent token uncancelled, never
  contacts the target, and leaves the independent relay participant running.
- **Recovery/Stop:** two complete cycles (twenty attempts), then one legitimate phone
  connection using the original decoded offer, without restarting the helper, replacing
  the grant or retrying the phone. Direct browser HTTP returns 200, `recovered`, and CSP.
  Explicit Stop releases both helper and phone UDP binds and closes the browser listener.
  The earlier single-wave test independently covers relayed HTTP recovery.

Both fixtures have a ninety-second outer test budget; these are finite loopback waves,
not an indefinite or rate-based flood benchmark. They do not measure CPU/RSS, cap QUIC's
pre-hook buffers, establish per-source fairness, or cover coordinated outgoing contention.
The [constructor-time follow-up](saorsa-preparation-admission.md) now closes the helper's
preparation bypass. The [phone phase gate](saorsa-phone-ingress.md) subsequently brings
the routing suite to 137 without further patch changes. Generic default-constructor
ingress, transport buffering and sustained contention remain separate policy work. Authorization races and malicious observation-frame coverage also remain open.

Generated-only mutations were caught and restored:

- Eight active handshakes: the first queued attempt received seven reply datagrams
  instead of zero.
- Thirty-second active deadline: queued work failed its bounded promotion check.
- Refusing the queued batch on active expiry: genuine client TLS failed with
  `CONNECTION_REFUSED`, rather than being mistaken for successful promotion.
- Sharing the caller's cancellation token instead of making a child: helper drop
  incorrectly cancelled the parent, and the test rejected it.

At the mixed-wave checkpoint, `verify-peer-routing.sh`: **124 passed**. `verify-helper.sh` passed the actual
terminal/QR/browser/cleanup suite. Traffic remained loopback-only; there was no Android
build/staging/install, public probe, deployment or source-pin change.

Logs: `/tmp/wpt-mixed-admission-{limit-mutation,limit-final-mutation,expiry-mutation,queued-refusal-mutation,parent-mutation,drop-green,green,restored-repeat,full,helper}.log`.
