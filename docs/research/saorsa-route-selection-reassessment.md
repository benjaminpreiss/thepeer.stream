# Reassessment: reuse upstream route selection, not an imagined “best route” switch

## Conclusion

**Do not replace our router with the pinned `P2pEndpoint::connect_with_fallback` as a
one-line fix. Do realign candidate discovery/planning with the inspected Saorsa-core
model.** The missing local address is our integration gap, not evidence that Saorsa's
UDP transport cannot connect efficiently on LAN.

There are three distinct layers:

1. Saorsa transport mechanisms and its optional older fallback cascade.
2. Saorsa-core's typed-address discovery, contextual candidate planning and dialing.
3. Our QR invitation, one-shot application authorization and Stop/teardown adapter.

We reuse (1), but did not integrate (2). Describing our custom three-stage loop as the
same selection logic used by Autonomi was misleading. Conversely, replacing our loop
with the older transport cascade would **not** reproduce the inspected core policy.

The original review below was source-only: it made no application changes, builds or
public probes. The subsequent implementation is summarized next; the original comparison
and recommendations retain their review-time context.

## Selected application policy

The user selected **Direct first, Relay as fallback** after this review.
The intended application path retains eligible local and observed-public Direct
candidates and falls back to the computer's prepared MASQUE contact only after
bounded no-response outcomes. Coordinated punching is not an intermediate stage
in this selected path. Authentication/protocol errors and Stop remain terminal.
This deliberately differs from core's generally relay-first policy outside LAN.

**Subsequent implementation:** `native/src/route_plan.rs` narrowly adapts core's local
address classification/shared-WAN rule under its MIT option; it does not import a DHT
or change the transport pin. `connect_direct_or_relay` is now the JNI path. It acquires
an ordinary phone observation, retires that endpoint/runtime, then executes a bounded
plan with separately owned candidate attempts and unchanged pin/grant/Stop semantics.
The old `connect` is host-review-only. Explicit observer failure is terminal; there is
no observer failover or guarantee of LAN service when a requested observer is down.

The helper retains bound and observed addresses; `wpa2:` adds flags 4/8 for bound/observer,
with a 245-byte text bound. Java/JNI bounds and source version **0.6-direct-relay-preview**
were updated. See [current offer](../ROUTING-OFFER.md) for exact semantics.

Host evidence: 157 library/routing tests (including historical review tests), pure-planner
checks, scanner/staging checks and helper QR→Java/JNI→HTTP/Stop passed. The nine planner
checks can run without Cargo via `verify-candidate-plan.sh`. No NDK/APK build, installation
or public probe was run for this implementation. The installed APK is unchanged; the
new helper requires an operator native/APK rebuild. Phone context is only an IP hint,
not a claim that a later socket has the observation phase's NAT mapping.

## Source scope

- Our installed transport pin: **0.37.0**, `e2f08388088313c82cac393afa6ca27b9ce00516`.
  Read the cached, unmodified Git checkout and our promoted patch stack.
- Inspected Autonomi-family node: `ant-node` at
  [`d07913de622f76133306a92edcb69a7dca831c96`](https://github.com/WithAutonomi/ant-node/blob/d07913de622f76133306a92edcb69a7dca831c96/Cargo.toml).
- Inspected core: [`75bb32bdb3476afe17d7f5248f27a8a216a7f326`](https://github.com/WithAutonomi/saorsa-core/blob/75bb32bdb3476afe17d7f5248f27a8a216a7f326/Cargo.toml),
  depending on transport **0.38.0**, not our pin.
- The core adapter was fetched again at that exact revision and matched the previously
  captured file. The contextual DHT selector and accepted ADR-016 were fetched at that
  same revision. These are **frozen source observations**, not a claim about today's
  deployed binaries or latest upstream HEAD. No version upgrade is implied.

## 1. What the pinned transport actually does

The actual implementation is more specific than its old sequential IPv4→IPv6 doc comment:

- `connect_with_fallback` accepts **one IPv4 and one IPv6 target**, not an arbitrary
  local/public candidate set. Its inner method races the supplied direct addresses using
  Happy Eyeballs, defaulting to IPv6 preference and a 250ms stagger. It then tries
  coordination and outgoing relay fallback when configured/enabled.
- It fills coordinator/relay candidates from configured or already connected peers.
  It does not discover an omitted target LAN address. Two IPv4 target addresses cannot
  both be supplied through its public IPv4/IPv6 arguments.
- It returns the first successful transport connection according to that strategy,
  not the route with measured minimum application latency or maximum throughput.
- Direct errors, including first-response/handshake errors, are converted to strings;
  exhaustion advances the strategy. Hole-punch errors/timeouts can trigger a direct
  retry and then further rounds/relay. This is not our `Some / None / terminal Err`
  classification.
- Direct success is finalized into endpoint maps, monitoring and a uni-stream reader.
  The return value is `PeerConnection` metadata plus `ConnectionMethod`, not a newly
  transferred exclusive raw QUIC handle.

Primary sources:
[actual cascade and finalization](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/p2p_endpoint.rs#L1983-L2584),
[Happy Eyeballs defaults](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/happy_eyeballs.rs#L78-L105),
[racing/error/cancellation implementation](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/happy_eyeballs.rs#L229-L457).

### Relay semantics differ too

The cascade's `try_relay_connection` establishes an **outgoing carrier**, creates a
virtual relay endpoint, then dials the supplied target through it. Our current phone
Relay leg instead dials the computer's **already allocated relay contact using ordinary
UDP**. The computer owns the carrier. Substituting the former can introduce a second
carrier when the computer is already relay-fronted; it is not just a different selector
for our current single-carrier path.

Source: [outgoing relay path](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/p2p_endpoint.rs#L2728-L2880).
This source difference is not a claim that every possible two-carrier arrangement fails.
Our historical two-carrier public attempt failed and remains unexplained.

## 2. What the inspected Saorsa-core call path does

### It normally disables that transport cascade

The core adapter constructs `P2pLinkTransport` with `StrategyConfig::direct_only()`.
`connect_to_peer_authenticated` calls `dial_addr` for a selected address. Its comments
explicitly assign typed-address selection to core and disable per-dial punching and
in-cascade relay fallback. A relay-allocated address can therefore be dialed by this
“direct-only” transport operation while remaining **logically a Relay route**.

Sources: [construction](https://github.com/WithAutonomi/saorsa-core/blob/75bb32bdb3476afe17d7f5248f27a8a216a7f326/src/transport/saorsa_transport_adapter.rs#L575-L608),
[dialing](https://github.com/WithAutonomi/saorsa-core/blob/75bb32bdb3476afe17d7f5248f27a8a216a7f326/src/transport/saorsa_transport_adapter.rs#L880-L925).

### Its candidate planner has LAN context that ours lacks

`DhtNetworkManager::select_dial_candidates_with_context` receives typed candidates and
local address context. It builds a bounded plan (at most five entries):

- At most one relay, excluding a relay endpoint on the node's own WAN IP.
- At most one best WAN candidate per family; Direct ranks above Unverified.
- At most one best LAN candidate per family, scored using local IP/prefix context.
- **If a non-LAN/non-relay peer address shares a locally known WAN IP, put available LAN
  candidates first.** Then relay, then WAN candidates.
- Otherwise relay precedes WAN; LAN is used only when no WAN/relay alternative exists.

The live context uses **non-relay** external addresses plus listen addresses; an owned
relay's public IP is not treated as the client's WAN identity. Core additionally filters
LAN addresses on storage unless the publication arrived over LAN or appears to share WAN.
Source: [context acquisition and storage filtering](https://github.com/WithAutonomi/saorsa-core/blob/75bb32bdb3476afe17d7f5248f27a8a216a7f326/src/dht_network_manager.rs#L3646-L3698).

Shared WAN is a heuristic, not proof that two peers share a usable LAN—CGNAT and network
segmentation exist. The function cannot select a LAN address absent from its input.
This also means core is **not universally Direct→Punch→Relay**: away from its contextual
LAN preference, the planner intentionally prefers relay for connection reliability.

Sources: [context and LAN scoring](https://github.com/WithAutonomi/saorsa-core/blob/75bb32bdb3476afe17d7f5248f27a8a216a7f326/src/dht_network_manager.rs#L339-L429),
[bounded contextual plan](https://github.com/WithAutonomi/saorsa-core/blob/75bb32bdb3476afe17d7f5248f27a8a216a7f326/src/dht_network_manager.rs#L4357-L4485),
[live dial path](https://github.com/WithAutonomi/saorsa-core/blob/75bb32bdb3476afe17d7f5248f27a8a216a7f326/src/dht_network_manager.rs#L3550-L3689).

The surrounding network chooses explicit caller addresses or authoritative owner-published
sets, with saved/DHT candidates where applicable. Core also supplies dial deduplication,
failure suppression and authenticated peer/channel handling. These are network-node
behaviors; they are not all desirable for a fresh one-shot QR session.

Source: [candidate provenance](https://github.com/WithAutonomi/saorsa-core/blob/75bb32bdb3476afe17d7f5248f27a8a216a7f326/src/network.rs#L1940-L2045).

The adapter additionally has dual-/split-stack dialing: the dual-stack branch follows
caller order sequentially; the split-stack branch can race family groups. The method's
name alone should not be read as “race every candidate and measure the best route.”
Source: [adapter candidate dialing](https://github.com/WithAutonomi/saorsa-core/blob/75bb32bdb3476afe17d7f5248f27a8a216a7f326/src/transport/saorsa_transport_adapter.rs#L2261-L2412).

### Do not mistake an old ADR for current LAN policy

ADR-014 explicitly described losing LAN optimization, but marks itself superseded.
[Accepted ADR-016](https://github.com/WithAutonomi/saorsa-core/blob/75bb32bdb3476afe17d7f5248f27a8a216a7f326/docs/adr/ADR-016-canary-gated-proactive-relays.md)
records provisional proactive allocations, canary-gated publication and owned teardown.
Use the actual contextual selector above for LAN ordering, not ADR-014's old simplification.
Importing core's relay publication machinery would also mean DHT/witness/maintenance work,
not merely replacing a function in our phone client. This review authorizes none of that
network activity.

## 3. Compatibility with our security and lifecycle requirements

| Requirement | Unmodified upstream replacement |
| --- | --- |
| Local + observed-public target candidates | Pinned fallback accepts only one address per family; our offer omits local when observing a participant. Neither problem is fixed by swapping the call. |
| Actual computer pin before grant | TLS is upstream's job, but intended-computer authorization remains ours. We must check the exact retained handle used for application streams. `authenticated: true` metadata is not the QR pin check. |
| Only no-response permits fallback | Stock cascade/Happy Eyeballs advance after general errors; they do not expose our typed terminal/no-response decision. |
| One helper winner | Our helper hands its unique inspector to the first completed TLS winner. Racing several independently authorizing connections is not a drop-in: client/server winners can differ. |
| Owned coordinated handoff | Our constructor-installed phone gate is Dormant→Armed→Closed; upstream punching polls registered connections. Our gate returns an owned connection rather than registering it. The old cascade cannot just be inserted underneath it. |
| Stop and complete loser cleanup | Happy Eyeballs aborts tasks on success/drop, which is useful. It does not await all aborted tasks before returning or provide our endpoint/runtime teardown receipt. |
| Existing single-carrier relay | Keep the target's allocated contact distinct from an outgoing relay server; otherwise fallback can change the topology. |

Current relevant implementation:
[selector/helper winner](../../crates/tunnel-core/src/peer_route.rs),
[typed attempt outcomes](../../crates/tunnel-core/src/peer_connect.rs),
[phone ingress refusal](../../crates/tunnel-core/src/phone_ingress.rs),
[private runtime ownership](../../crates/tunnel-core/src/route_runtime.rs),
[actual-connection authorization](../../crates/tunnel-core/src/application.rs).
The former coordination owner has been removed.

**Stream-ownership caveat must be stated precisely.** The pinned P2P reader accepts
**unidirectional** streams; the NAT connection monitor only waits for closure. That alone
does not establish a conflict with our bidirectional HTTP streams. Do not justify the
custom selector with a blanket claim that every stock connection consumes our HTTP data.
The concrete differences are registered/shared ownership, extra tasks and the caller's
retained-handle/cleanup contract. Pinned `P2pLinkTransport::dial_addr` obtains its handle by
address lookup after the cascade, then extracts identity from that retrieved connection.
That is not intrinsically a pin bypass, but is not our existing ownership contract.

Sources: [uni reader](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/p2p_endpoint.rs#L1063-L1135),
[closure monitor](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/nat_traversal_api.rs#L4580-L4604),
[link dial/handle lookup](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/link_transport_impl.rs#L680-L727).

## 4. Recommendation and reuse boundary

### Recommended next change

1. Carry a bounded **typed candidate set**, retaining the helper's local bound address
   alongside the participant-observed address and the separately tagged relay contact.
   Observation is not proof of public reachability. Do not discard provenance or pretend
   every private address is reachable from every phone.
2. Align candidate planning with core's **context-aware LAN/WAN distinction**, rather than
   inventing another transport traversal mechanism. Establish what trustworthy local/WAN
   context the phone actually has; our current wildcard bind alone does not provide it.
   No user-facing LAN toggle is needed.
3. Keep our small application-owned dialing/authorization/Stop adapter and current
   single-carrier primitive. Prefer bounded ordered attempts initially; do not blindly
   parallelize full application joins against the one-winner helper.
4. Be explicit about the policy choice: preserving the requested public
   Direct→Coordinated→Relay order while borrowing core's candidate model is **not identical
   to core's generally relay-first ordering**. Adopt core's complete ordering only by an
   explicit decision, not under the label “best available.”

This is **design/algorithm reuse**, not a claim that the private core selector can simply
be called from our 0.37 dependency. That function and its context are internal to core;
there is no ready public socket-list planner interface here. A small attributed port or
an upstream extraction would be an explicit choice. Importing all of saorsa-core or
upgrading transport to 0.38 just to reach that private method is not recommended.

### What could be reused more directly later

The pinned generic Happy Eyeballs scheduler already accepts an arbitrary address slice
and a caller-supplied dial future. It is a plausible extraction/reuse starting point,
not an unchanged replacement: typed fatal outcomes, stop precedence, awaited loser
cleanup and helper winner semantics must be resolved. Prefer a narrow additive seam
with unchanged upstream defaults over duplicating the entire fallback engine.

### Decisions before implementation

- Candidate schema/version and bounds: current `wpa2:` has one mandatory Direct address;
  Java/JNI bounds are 103–195 text bytes. Adding candidates requires a deliberate helper,
  codec and APK migration, not editing a live offer or silently reinterpreting bytes.
- How to acquire/validate phone context without making offline LAN connectivity depend
  on successful public bootstrap. Shared-WAN/prefix hints never replace TLS pin checks.
- Whether to preserve our explicit route priority or intentionally adopt core's different
  relay-first network policy.
- Preserve the current explicit single-family contract initially. Core's dual-/split-stack
  support is not inherited merely by copying candidate ordering; changing that requires a
  separate owned endpoint/family decision, not implicit address synthesis.
- Keep fallback classification and cleanup as separate requirements even if a planner is
  reused. Do not turn an authentication error into “try another address.”

The immediate user evidence is that LAN Direct loads quickly while the current relayed
path loads slowly. That isolates a useful comparison but does not identify the relay's
latency/throughput bottleneck. Candidate selection work should avoid needless relay use;
it does not promise to make unavoidable cellular relay traffic fast. No further generic
stress-testing prerequisite is proposed by this review.
