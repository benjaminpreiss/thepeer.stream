# Coordinator-scoped observed candidates

## Result and scope

The first candidate-acquisition slice now learns the helper's direct and coordinated
target hints from the **selected coordinator's ordinary-UDP connection**, rather than
publishing the local bind as Direct and sampling an arbitrary endpoint-wide observation.
The format remains `wpa2:`; no new upstream patch or Android integration change.

An explicitly requested coordinator must bootstrap and provide an eligible observation.
The observation wait is bounded to **five seconds**, polling every 20 ms because a
missing snapshot can mean contention or an in-flight frame. Stop, control loss,
invalid observation and timeout are terminal preparation results. No offer is returned
by substituting local bind on those failures. This is a preparation budget, not a healthy
session limit, and does not change the phone selector's conservative fallback rules.

**145 routing tests passed** after the [paced contention case](saorsa-paced-contention.md).
The helper process/terminal suite also passes after approved incremental-cache cleanup
resolved a disk-full compilation failure. Earlier checkpoints: 144 at the
[authorization/Stop follow-up](saorsa-authorization-stop.md) (137 at the
[phone-ingress follow-up](saorsa-phone-ingress.md), 130 at the
[constructor-time follow-up](saorsa-preparation-admission.md), 124 at the
[mixed-ingress follow-up](saorsa-virtual-admission.md#follow-up-repeated-mixed-ingress-waves-and-drop),
122 at the [virtual-admission extension](saorsa-virtual-admission.md), 119 at the
[virtual-winner follow-up](saorsa-route-negative-matrix.md#follow-up-unauthorized-virtual-winner-ownership)).
The [observer safety follow-up](saorsa-route-negative-matrix.md#follow-up-observer-control-loss-and-ineligible-reports)
passed at 118 tests (117 at [final relay Stop](saorsa-route-negative-matrix.md#follow-up-stop-during-the-phones-final-relay-dial),
116 at the [family-policy/relay-only slice](saorsa-route-family-policy.md), 114 after
structural preflight/IPv6 below, 112 at initial acquisition). All traffic in
this slice was loopback. This is address-selection/ownership evidence, not public NAT,
coordinator reachability from cellular, or Android evidence.

## Primary-source findings

Source reviewed directly at pinned `e2f08388088313c82cac393afa6ca27b9ce00516` (0.37.0):

1. [`P2pEndpoint::external_addr`](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/p2p_endpoint.rs#L1573-L1576)
   delegates to an endpoint-wide snapshot and suppresses its error into None.
2. [`get_observed_external_address`](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/nat_traversal_api.rs#L5955-L6000)
   prefers configured known-peer connections, then returns an observation from **any**
   connection in the map. It does not select our configured coordinator by itself.
   It is unsuitable for asserting the provenance of this offer's coordinator target.
3. [`Connection::observed_address`](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/high_level/connection.rs#L778-L800)
   reports that specific connection's observation. None can mean no frame yet,
   incomplete handshake, disabled discovery, or a failed nonblocking lock acquisition.
   A single immediate read followed by local-address substitution is not acquisition.
4. [Observation sending](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/connection/mod.rs#L5500-L5545)
   reports the peer address on that QUIC path via an encrypted OBSERVED_ADDRESS frame.
   This is a peer's network observation, not proof that a different peer can reach it.
5. [`prepare_proactive_relay`](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/nat_traversal_api.rs#L6027-L6040)
   establishes a separate relay path while retaining the original UDP endpoint.
   A relay allocation contact must not be relabelled as an ordinary direct observation.

The required per-connection observation/close interfaces already exist. No missing
capability required expanding the review patch stack.

## Adapter changes

`apps/android-controller/native/src/peer_route.rs` now:

- Bootstraps the explicitly configured coordinator as before.
- Resolves and retains its control connection once, rejects a virtual-socket handle,
  then reads observations only from that handle. This control lookup carries no grant;
  application identity/authorization still uses the actual application connection.
- Waits for observation with biased Stop/control-loss handling. Checks recorded close
  state again before accepting a snapshot.
- Canonicalizes an internal IPv4-mapped representation to IPv4, then applies existing
  candidate validity checks. This does not relax the QR decoder's alias rejection.
- Uses the observed address for Direct and for the separately tagged coordination
  target. Matching address values do not merge the two route provenances.
- Leaves the relay allocation contact separate and prepares it only afterwards.

At this initial slice, without a coordinator the explicit bind remained the direct
hint, including relay-only preparation. The [follow-up](saorsa-route-family-policy.md)
now uses the relay participant's ordinary-UDP observation for Direct when no coordinator
is configured, without enabling coordination. With neither participant, bind remains
the direct hint. No STUN service, network interface guessing, UPnP or advertisements
were added. The helper command still rejects wildcard binds. The single Direct slot
now favors the coordinator-observed mapping when configured; it is not a ranked list
of both local and observed addresses.

Coordinator availability policy remains conservative: configuring one requests its
support. Bootstrap/observation failures do not silently omit it or initiate participant
rotation. After preparation, an already successful direct session does not depend on
the coordinator remaining up (existing regression remains green).

## Real-boundary tests

`peer_route_candidate_tests.rs` tests the already approved preparation/offer/connection/
browser/Stop seams. No private grant setter, fabricated observation or mocked route
outcome is used.

### Source translation → decoded offer → Direct HTTP

The same test now runs on IPv4 **and IPv6 loopback**. Three UDP sockets provide separate
control and incoming forwarding legs. The target and browser HTTP sockets remain IPv4
localhost; the helper/coordinator/phone QUIC path and observed hint use the tested family.
The coordinator sees a different source port from the helper's local bind. Real QUIC
packets are forwarded unchanged. The test checks the produced Direct hint is the
observed mapping, round-trips through the real codec, connects through that hint, and
serves browser HTTP 200/body/CSP on **Direct**. Stop completes and the helper bind can
be reused. The assertion was red before implementation (local bind instead of mapping).

This is a controlled address-translation fixture, not evidence about real NAT filtering,
mapping dependence, hairpinning, port prediction, or mapping expiry.

### Withheld observations after genuine TLS

A bounded synthetic forwarder passes QUIC v1 Initial/Handshake packets and removes
server-to-helper 1-RTT packets, including a coalesced short-header tail. It never decrypts
or constructs protocol frames. The real coordinator accepts TLS; encrypted observations
cannot arrive. A no-loss positive control first proves normal acquisition through the
same fixture. Further cases establish:

- Silence produces `route-observation-timeout`, no offer and no target connection.
- A separate real peer sends an observation during that wait (actual transmitted frame
  counter checked). Its observation cannot substitute for the selected coordinator's.
- Stop after TLS acceptance and observed 1-RTT loss returns `peer-cancelled`, not an offer
  or an observation-timeout result.
- Control closes and the helper UDP bind is reusable after completion.

Generated-only sensitivity checks were caught and restored:
- Switching the read back to `endpoint.external_addr()` made the unrelated-peer case
  return an offer, failing the test.
- Ignoring Stop inside acquisition made the Stop case return observation timeout,
  failing the test.

### Discarded fixture assumptions

An initial attempt to create silence through remote configuration was not a valid
fixture. The pinned [`enable_address_discovery(false)`](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/config/transport.rs#L409-L428)
explicitly ignores the legacy boolean and enables SendAndReceive. Explicit configuration
attempts also did not provide the intended clean silent connection: None caused a
protocol close in the trial, while ReceiveOnly still produced an observation. See the
[connection initialization](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/connection/mod.rs#L438-L453)
and [negotiation code](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/connection/mod.rs#L5897-L5927).
Those assumptions/tests were replaced with real packet loss, not patched into upstream
or counted as passing coverage of unsupported-participant behavior. Negotiation edge
cases merit separate review; no security or interoperability fix is claimed here.

## Follow-up: structural eligibility before preparation

The library now validates helper bind, optional coordinator and optional relay before
creating an endpoint, generating a grant, or contacting any participant. This brings
native callers under the existing host CLI's address restrictions instead of relying
on a later encoder/connection failure. Bind port zero remains allowed; participant
ports must be nonzero. Wildcards, multicast, limited IPv4 broadcast, mapped IPv6,
IPv6 link-local, and nonzero IPv6 scope/flow metadata are rejected as `route-input`.
A pre-cancelled operation still returns `peer-cancelled` first.

Two new regressions at the existing preparation seam:

- An invalid relay contact must not initiate even a valid coordinator bootstrap.
  Before the change, a real loopback sink received the Initial; the test drained
  cancellation before reporting red. After the change, preparation rejects input,
  the bind remains reusable, and the target is untouched.
- Unrepresentable bind / invalid coordinator configuration must be rejected before
  endpoint creation. Before the change a mapped-loopback bind actually returned a
  helper/offer. Cases now cover wildcard inputs, zero participant ports, invalid
  address classes and IPv6 scope/flow. Invalid non-loopback coordinator cases hold
  the helper bind occupied: bypassed preflight fails binding rather than emitting
  non-loopback traffic. Removing coordinator validation in a generated copy changed
  the result to `route-endpoint`, and was caught.

The translated-address HTTP test passed immediately over IPv6 with the existing
transport implementation: no IPv6 transport fix is claimed. A generated mutation
substituting local bind **only for IPv6 observations** failed that test after its
IPv4 case passed. Both mutations were restored before the full green run.

These checks establish representability and explicit exposure, **not public Internet
eligibility**. Private, loopback and ULA hints are not silently reclassified as public.
No global-only mode or hard-coded public-range classifier was added. Same-family
IPv6 loopback success does not establish dual-stack selection, NAT64, public IPv6
reachability, IPv6 coordination punching, or IPv6 relay allocation behavior.

Follow-up logs: `/tmp/wpt-eligibility-{red,green,bind-red,bind-green,candidates,ipv6,ipv6-mutation,coordinator-mutation,full,helper}.log`.

## Verification and remaining work

Fresh commands:

```sh
bash experiments/direct-traversal-review/verify-peer-routing.sh
JAVA_HOME=/path/to/installed/jdk \
  bash experiments/direct-traversal-review/verify-helper.sh
```

Logs: `/tmp/wpt-candidates-{red,green,loss,provenance,global-mutation,stop-mutation,full,helper}.log`.
The current full suite includes five candidate/preflight tests, a selected-family
coordination/relay regression, and the existing direct/coordinated/relay,
authorization, cancellation, and active-stream cleanup cases. No SDK build, staging,
installation, deployment or public probe occurred. Patch files and dependency revision
are unchanged.

The runner now requires working IPv4 and IPv6 loopback; it does not silently skip IPv6.
Helper wildcard binds are deliberately rejected, including when a coordinator is set.

The explicit-family and relay-only observer policies are now implemented in the
[follow-up](saorsa-route-family-policy.md). Still pending: public eligibility and automatic
dual-stack selection; multiple candidates and destination-dependent mappings; mapping freshness across
preparation and QR scanning; malformed/replayed observation-frame cases beyond the now-covered authenticated
close/ineligible-address cases; hostile virtual-listener/overload review; approved bounded public tests;
patch promotion and Android migration. An observation alone is not a global-routability
or coordinator-service guarantee.
