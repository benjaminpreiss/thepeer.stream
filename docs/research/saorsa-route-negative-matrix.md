# Route adapter: TLS, cancellation and overlapping handshakes

Follow-up to [active-stream teardown](saorsa-route-stream-teardown.md). This slice
adds four integration regressions at the approved helper/phone seams. They passed
against the existing implementation; **no production behavior or upstream patch
changed**. No public participant, Android build, staging or deployment was involved.

## Real-traffic cases added

`apps/android-controller/native/src/peer_route_negative_tests.rs` exercises public
helper preparation, offer routing hints, phone connection and Stop/cleanup:

1. **Partial direct TLS with alternatives available.** A UDP forwarder delivers real
   client traffic and exactly the first server datagram, then drops subsequent server
   replies. Forwarded/dropped notifications prove this is not a silent destination.
   The selector returns `peer-handshake-timeout`, never a session, despite a configured
   working coordinator and prepared relay. The localhost target receives no connection.
2. **Stop after an actual coordination instruction.** Direct is silent. An independent,
   bootstrapped participant uses the existing public handshake hook to hold the real
   `PeerDial` delivered by the coordinator. Its destination matches the phone socket.
   Only then is the phone token cancelled. Connection returns `peer-cancelled`, the
   phone UDP port is released, and the intended target is untouched despite an available
   relay. The helper's separate parent token stays uncancelled.
3. **Stop during helper relay preparation.** A real participant completes bootstrap
   but holds the subsequent incoming handshake for the owned allocation carrier.
   Cancellation returns `peer-cancelled` rather than a helper/offer pair, releases the
   helper UDP socket and never contacts the target. This covers *helper preparation*,
   not cancellation during the phone's final relay-connect branch.
4. **Progress with unfinished handshakes.** Four independent UDP forwarding paths
   deliver client requests but discard every helper response. Actual helper replies
   establish that those handshakes started. None accesses the target; the legitimate
   caller still authorizes and serves browser HTTP 200 with the independent `overlap`
   fixture body, then stops. The assertion accepts either Direct or Coordinated rather
   than depending on scheduling or the implementation's exact admission capacity.

These peers are real QUIC endpoints. The held dial/incoming handles are fault injection
at the remote participant's public transport boundary, not mocks of route decisions,
private helper state, grants or crypto outcomes. Only the synthetic participant serves
relay traffic; helper, phone and probe roles remain non-serving with UPnP disabled.

## Sensitivity checks

Temporary edits in **generated adapter copies only** established that the regressions
can detect the policy failures they are intended to prevent:

- Converting terminal `connect_owned` errors into `Ok(None)` made the partial-TLS test
  fail its terminal-error assertion.
- Ignoring the coordination Stop token made the instruction-based cancellation test
  fail its bounded completion assertion.
- Ignoring Stop while preparing the relay carrier made the preparation-cancellation
  test fail its bounded completion assertion.

All diagnostic mutations were restored before the fresh suite. No transport patch,
Cargo checkout or production preparation was edited. The overlap case characterizes
existing behavior; no fix was needed for it.

## Verification

Fresh `bash experiments/direct-traversal-review/verify-peer-routing.sh`: **105 passed**
(previous 101 plus these four). All four new tests also passed two consecutive filtered
runs. Formatting and diff checks passed. The preceding slice's 96 legacy relay/JNI/
staging checks are historical results, not newly rerun claims for this test-only slice.

Logs:
- `/tmp/wpt-route-negative-tls.log`
- `/tmp/wpt-route-negative-coordinate.log`
- `/tmp/wpt-route-negative-relay-prepare.log`
- `/tmp/wpt-route-negative-overlap.log`
- `/tmp/wpt-route-negative-catchall.log`
- `/tmp/wpt-route-negative-stop-mutations.log`
- `/tmp/wpt-route-negative-full.log`
- `/tmp/wpt-route-negative-repeat-{1,2}.log`

## Follow-up: Stop during the phone's final relay dial

`peer_route_relay_cancel_tests.rs` adds a real preparation → encoded offer → phone
connection/Stop regression. It passed against the existing cancellation implementation;
no production fix was needed. Only test code and test-module registration changed.

A relay-only helper requests an actual loopback allocation. Direct points to a silent
UDP sink, and the encoded relay contact is routed through a UDP forwarder to the real
allocated contact. Only the public address record changes; the actual computer pin and
fresh grant remain unchanged in zeroizing buffers. No private grant setter or mocked
route result is used.

Three cases use the same fixture:

1. **Positive control:** forward all traffic. Selection reaches `Route::Relay`, the
   decoded offer authorizes real browser HTTP 200/body `control`/CSP, then Stop closes
   serving and the browser listener.
2. **No relay response delivered:** forward phone traffic into the real allocation,
   but hold every returning helper TLS datagram. Cancel only after an actual reply
   arrives at the allocated contact/forwarder.
3. **Partial relay TLS:** forward one genuine helper response, hold the next, then
   cancel. Synchronization is on network events, not a sleep guessing which route ran.

Both cancellation cases return `peer-cancelled`, not a session or unavailability.
The helper reports no authorization and the target receives no TCP connection. Real
queued Direct traffic proves the earlier attempt happened; the final relay endpoint's
phone port is owned before Stop and reusable after completion. Helper cleanup also
releases its local UDP port. The independent helper parent token remains uncancelled,
and the participant remains running until explicit fixture shutdown. This makes no
claim about reclaiming the participant's intentionally retained allocation reservation.

Ignoring Stop **only on the final relay branch**, in a generated adapter copy, was caught:
- With replies withheld, the result became `route-unavailable` instead of cancellation.
- With the partial-TLS case ordered first, completion exceeded the test's ten-second
  cancellation budget. The positive HTTP control still completed beforehand.

Both the implementation mutation and temporary case ordering were restored. Fresh full
routing suite: **117 passed** (previous 116 plus this regression). The real helper
process/terminal suite also passed freshly. Formatting/diff checks passed. No upstream
patch, public traffic, Android build/staging/install, or deployment occurred.

Logs: `/tmp/wpt-final-relay-stop-{first,mutation,partial-mutation,full,helper}.log`.
These cases cover response-wait and partial-TLS cancellation, not a separately controlled
post-TLS/grant-authorization race. Existing active-stream Stop/drop coverage is separate.

## Follow-up: observer control loss and ineligible reports

The existing observation-loss fixture now includes coordinator and relay-only control
closure after real TLS, while observations are still withheld. The remote closes its
actual connection with a large synthetic reason. The packet-loss proxy keeps small
observation-bearing packets lost but permits the larger encrypted close; it does not
forge or decrypt protocol frames. Both cases return `route-observer-control`, produce
no offer, leave the target untouched and release the helper bind. This covers an
explicit authenticated close, not instantaneous detection of a silently vanished peer;
the existing observation-timeout cases cover silence.

`peer_route_observer_tests.rs` adds an authenticated dishonest-observer fixture using
Saorsa's public [socket interface](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/high_level/runtime.rs#L54-L114).
The remote-only socket adapter presents a false source address to a genuine QUIC peer,
which emits its own authenticated/encrypted observation frames. Actual outgoing UDP
destinations are always rewritten back to the real loopback helper. Neither helper
state nor plaintext protocol frames nor cryptographic outcomes are injected. The
remote uses the ordinary raw-public-key builder with `PqcConfig::default()`.

An unchanged-address positive control prepares a helper with the correct hint. Then,
for both coordinator and relay-only observation, real reports carry:

- a zero port;
- unspecified IPv4 or IPv6;
- multicast IPv4 or IPv6;
- limited IPv4 broadcast;
- IPv6 link-local without usable scope.

All fourteen negative role/address cases return `route-observed-address`. Actual
transmitted observation counters are checked, no offer escapes, target TCP remains
untouched, helper bind is reusable, and the caller token remains uncancelled. Relay-only
cases also verify that no second incoming connection (allocation carrier) starts at
the participant. These are well-formed authenticated frames with ineligible addresses,
not malformed-wire, forged-signature, replay or global-reachability tests. A structurally
eligible but dishonest hint is still not proof of reachability or computer identity.
The [upstream receiver](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/connection/mod.rs#L5192-L5269)
normalizes and records the reported address; the adapter's eligibility check remains
necessary before returning an offer.

The fixture needs the public socket trait's `quinn-udp` types. Added **host-test-only**
`quinn-udp = "=0.6.3"`, already locked transitively. Lock comparison confirmed identical
package sets, versions and checksums; only the native package's test dependency edge was
added. No runtime dependency upgrade, upstream patch or Android integration migration.

Generated-only sensitivity checks, restored afterwards:
- Removing observation validation returned an offer for `127.0.0.1:0`, failing the test.
- Ignoring observer closure returned `route-observation-timeout` rather than the
  explicit control-loss error, failing the test.

No production behavior fix was needed. Fresh routing suite: **118 passed** (117 plus
the new observer regression; the two close cases extend an existing test). The real
helper process/terminal suite also passed freshly. All traffic was loopback; no SDK
build, staging/install, deployment or public probe occurred.

Logs: `/tmp/wpt-observer-{close-first,close-mutation,report-first,report-matrix,report-mutation,safety-full,safety-helper}.log`.

## Follow-up: unauthorized virtual winner ownership

`unauthorized_virtual_winner_cannot_reach_target_or_survive_stop_and_drop` in
`peer_route_relay_tests.rs` exercises actual TLS through an allocated relay contact.
The attacker receives only the contact from a decoded offer, never the grant. Five
cases cover idle-before-authorization and partial-record clients under both explicit
Stop and helper drop, plus an application request sent without authorization.

The partial cases transmit a length prefix and incomplete body without FIN/reset;
public client STREAM counters establish real transmission, not an injected helper
state. They do not establish the exact instant the remote parser consumes each byte.
No authorization acknowledgment or target TCP access occurs. Stop is bounded separately
from the authorization deadline. Both ownership exits close the retained peer, release
the helper UDP bind, and withdraw the relay contact while leaving the participant
running. The unauthorized request receives no application acknowledgment and ends with
`application-auth-denied`. Drop/session failure does not cancel the parent token.
Remote reservation/FD reclamation is still not claimed.

Generated-only mutations were caught and restored: ignoring helper Stop exceeded the
18-second teardown check; bypassing authorization sent an application acknowledgment.
No production fix was needed. Fresh **119 routing tests** and the real helper process
suite pass. No public traffic, Android integration migration/build, dependency change
or upstream patch change in this slice. Logs:
`/tmp/wpt-virtual-winner-{first,stop-mutation,auth-mutation,full,helper}.log`.

### Pre-TLS virtual admission gap — patch decision required

**Historical decision point at 119 tests.** The user subsequently approved the
[opt-in extension](saorsa-virtual-admission.md): 122 tests now cover bounded virtual
admission, expiration/real HTTP recovery and ordinary-only hook isolation. The earlier
finding and decision rationale below are retained; pre-hook ordinary ingress and
sustained overload remain separate limitations.

This test covers **completed TLS**, not bounded pre-TLS virtual admission. Source
inspection of the pinned [virtual accept loop](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/nat_traversal_api.rs#L6320)
shows a spawned task per incoming attempt, awaiting TLS before publishing the connection.
At this checkpoint the review hook intercepted ordinary UDP admission, not this loop. The
adapter therefore cannot apply its four-active/four-queued handshake policy here via
`accept_with_connection()`, which exposes completed connections.

Do not treat `.max_connections(12)` as an established transport admission bound. The
pinned configuration stores that value, but inspection did not find it enforcing this
accept path. The separate [QUIC `max_incoming` setting](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/config/mod.rs#L460-L474)
limits `Incoming` objects **before acceptance**, not the spawned active handshakes.
This is a source-level ownership/admission gap, not a demonstrated exhaustion attack.

Before expanding patches, decide a generic, opt-in early virtual-admission interface.
Keep quotas, deadlines and ownership in our adapter. Preserve ordinary-only behavior
for coordinated attempts; blindly feeding virtual ingress into every existing hook
could change route classification. Preparation-time registration and teardown also
need auditing so there is no unowned admission window. No such patch expansion has
been made at that checkpoint. The subsequent bounded red test caught the fifth
attempt starting TLS; the approved extension now intercepts virtual ingress before TLS.

## Follow-up: repeated mixed ordinary/virtual admission

The [bounded mixed-ingress follow-up](saorsa-virtual-admission.md#follow-up-repeated-mixed-ingress-waves-and-drop)
adds two tests: repeated shared-budget overflow/expiry followed by either original-offer
browser HTTP/Stop or a third saturated wave and helper drop. Queued clients must finish
real client-side TLS, not merely receive a delayed refusal; helper-side Finished remains
lost. Parent cancellation isolation and local UDP release are checked. Four generated
mutations are caught and restored. Fresh **124 routing tests** and the real helper
process suite pass, without another implementation, patch, dependency or Android change.
This is finite loopback recovery evidence, not fairness under an indefinite flood.

## Preparation admission bypass — closed in review

The [ordinary preparation diagnostic](saorsa-preparation-admission.md) reproduced five
client TLS completions while helper-side Finished was withheld during bootstrap. The
ready-helper control admitted only four. The approved constructor-time hook now installs
before ordinary acceptance and the helper refuses unsolicited ingress until ready.
Three preparation tests are promoted into the green suite; three generic constructor/
provenance tests cover initial ownership, failure, removal and actual foreign observations.
The helper's foreign-observer fixture now verifies refusal, while a public transport
fixture retains global-versus-selected observation evidence. Fresh **130 routing tests**,
**91 coordinated-client tests** and the real helper suite pass. No exhaustion, public
readiness or whole-transport connection-cap claim.

## Phone ingress — adapter phase gate

The [phone-ingress follow-up](saorsa-phone-ingress.md) reproduced unsolicited ordinary
TLS on a healthy Direct phone. A constructor-installed adapter gate now remains dormant
through Direct/Relay and coordinated bootstrap, arms only the owned coordinated attempt,
and permanently closes on handoff/revocation. No new transport interface or patch change.
Seven new regressions cover setup, bootstrap Stop/drop, all three healthy route refusals,
actual browser HTTP and session cleanup. Fresh **137 routing tests**, **91 coordinated-
client tests** and the helper process suite pass. Missing-hook, premature-arm and ignored-
closure mutations are caught/restored. This is not sustained-load or physical-phone proof.

## Authorization/Stop ordering — seven real-wire checks

The [authorization/Stop follow-up](saorsa-authorization-stop.md) holds encrypted client
or server application traffic while retaining actual TLS. Both directions have browser
HTTP positive controls. Phone Stop/drop while awaiting acknowledgment, helper Stop before
client application delivery, and Stop after successful authorization but before a queued
browser is served all retain target/socket ownership. Four ACK-release/Stop event-order
trials permit only cancellation or a stopped session. Fresh **144 routing tests** and the
actual helper suite pass; three generated cancellation mutations are caught/restored.
No production behavior change; not exhaustive interleaving or sustained-load evidence.

## Paced mixed-ingress contention — fixed-budget recovery

One [100-attempt case](saorsa-paced-contention.md) adds two paced overload windows,
queued-work preservation through expiry, original-offer browser HTTP recovery and Stop/
socket release. A queue-discard mutation is caught/restored. Fresh **145 routing tests**
and the helper process suite pass; the latter required approved incremental-cache cleanup
after a disk-full compilation failure. Arrivals pause across expiry/promotion. This is not continuous-load
fairness, CPU/RSS measurement or coordinated outgoing contention. No production changes.

## Remaining security work and next interface decision

**Follow-up:** the [target offer codec](../ROUTING-OFFER.md) now supplies the public
encode/decode interface and wrong-grant tests across all three routes (110 tests).
The deferral below records the earlier slice. Legacy compatibility is not a requirement;
Android integration source remains deferred by explicit instruction.

This is not an exhaustive negative matrix. Final relay response-wait/partial-TLS
cancellation and observer close/ineligible-address reports are now covered above.
Still pending are malformed/replayed observation frames, further cryptographic-rejection
combinations at the selector, further post-TLS authorization interleavings, sustained-flood fairness,
coordinated outgoing dials under contention, generic default-constructor ingress and
transport buffering. The virtual-admission
extension closes the hook-ownership gap above; it does not establish general availability
under attack. Post-TLS hostile-winner isolation and ownership exits are also covered. A post-response TLS
timeout is not the same test as a forged TLS signature; existing lower-level signature
and authorization tests remain in the full suite.

The current public `Offer` is opaque and only produced by a helper with a matching
fresh grant. Therefore a syntactically valid **wrong grant cannot be supplied through
this public interface**. No private-field mutation or test-only grant setter was added
to pretend otherwise. Selector-level bad-grant and malformed-capability tests should
use the forthcoming validated/versioned offer decoding boundary; lower-level wrong-
grant tests already exercise application rejection, but are not claimed as equivalent
selector coverage.

The next design step is that untrusted offer input boundary (including candidate
provenance and secret handling), not Android activation. Public NAT acquisition, patch
review/promotion, Android/JNI wiring and actual cellular → Wi-Fi remain separate work.
