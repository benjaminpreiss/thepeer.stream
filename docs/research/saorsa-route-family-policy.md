# Explicit family policy and relay-only observation

## Current adapter decisions

This is a bounded host-policy slice, not automatic dual-stack or NAT64 support.
The public preparation/offer/connection/browser/Stop seams and `wpa2:` format are unchanged.

### One explicit local family per operation

- Helper preparation owns one ordinary UDP endpoint. Every configured coordinator and
  relay participant must match the explicit helper bind's family. Validate all of them
  before creating an endpoint or contacting even an otherwise usable participant.
- A phone `connect(offer, bind, stop)` selects one family via `bind`. This applies to
  wildcard binds too: `[::]:0` does not implicitly request an IPv4 attempt.
- Validate the syntax/representability of **all** offer contacts first, as before.
  A malformed unused contact is still terminal.
- Check family compatibility for the **selected** route before creating its endpoint:
  Direct needs the direct contact; Coordinated needs both coordinator and target;
  Relay needs the allocated relay contact. Incompatibility returns `route-family`.
- A structurally valid unused hint in another family does not poison a healthy Direct
  session. If selection reaches that incompatible route after actual silence on the
  previous route, it fails there. It is not silently skipped.
- Preserve the rule that only a conservatively classified `Ok(None)` advances. A known
  local family mismatch must not masquerade as an unanswered remote attempt.

This is a family-compatibility check, not proof of interface availability or an OS route.
It does not silently widen a concrete bind, choose another interface, race endpoints,
try both families, or synthesize a translated destination.

### Relay-only preparation learns a direct hint

Observer priority is explicit:

1. With a coordinator, use its ordinary-UDP observation for Direct and the separately
   tagged coordination target. A second relay participant does not overwrite it.
2. Without a coordinator but with a relay participant, bootstrap its **ordinary UDP
   connection**, acquire its observation, then request the relay allocation. Use the
   observation for Direct; keep the allocation contact exclusively as Relay.
3. With neither participant, keep the explicit bind as the direct hint.

Using a relay participant as observer does **not** enable coordinated punching. The
relay-only offer still has no coordination record. Missing/invalid observation, lost
control, Stop and bootstrap failure remain terminal preparation outcomes, not permission
for a local-bind substitute or participant rotation. The five-second observation budget
and existing allocation/cleanup budgets are setup limits, never session lifetimes.

The shared acquisition implementation is now `observed_candidate`; control/provenance
failures are named `route-observer-control` / `route-observer-provenance` rather than
coordinator-only diagnostics. No compatibility alias was added.

## Primary-source basis

Inspected pinned Saorsa `e2f08388088313c82cac393afa6ca27b9ce00516` (0.37.0), not a newer release:

- [`Endpoint::connect_with`](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/high_level/endpoint.rs#L375-L408)
  rejects an IPv6 destination on an IPv4 endpoint immediately. On an IPv6 endpoint,
  it converts IPv4 destinations through `ensure_ipv6` instead. These paths are asymmetric.
- [`ensure_ipv6`](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/high_level/endpoint.rs#L1041-L1046)
  uses IPv4-mapped IPv6 addresses. That is not discovery of a network's NAT64 prefix.
  Neither this conversion nor the endpoint's IPv6 flag proves a particular bound
  socket/interface can send to the destination.
- [`establish_owned_relay_session`](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/nat_traversal_api.rs#L4755-L4811)
  creates a dedicated carrier connection rather than borrowing the ordinary peer
  connection. Its ownership is kept separately from ordinary peer-table lifetime.
- [`connect_new_to_relay`](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/nat_traversal_api.rs#L4964-L4994)
  creates that connection through the main UDP endpoint. A dedicated QUIC connection
  does not imply another local UDP socket.
- [`prepare_proactive_relay`](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/nat_traversal_api.rs#L6027-L6150)
  leaves that UDP endpoint intact and creates another accepting QUIC endpoint backed
  by the virtual relay socket. The allocated public contact belongs to that relay path,
  not to the original UDP observation.

The existing per-connection observation and owned relay interfaces suffice. No upstream
patch expansion, dependency update or Cargo-cache edit was required. See also the
[observation acquisition findings](saorsa-route-candidates.md).

## Real-seam evidence

**116 routing tests and the real helper process/terminal suite passed.** Loopback only;
IPv4 and IPv6 loopback must both work. No Android build/staging/install or public probe.

### Relay-only source translation

The existing real UDP translation → offer codec → Direct browser HTTP test now also
runs with **only** a relay participant. The participant sees a source port distinct
from the helper bind; real bootstrap and carrier connections traverse that translation.
The test checks that:

- Direct is the observed mapping, not local bind or the allocation contact.
- No coordination record was implicitly enabled.
- The decoded offer serves HTTP 200/body/CSP on Direct, with normal Stop/cleanup.

This was red before implementation: the relay-only offer still contained local bind.
IPv6 relay allocation is not covered by this extension; its relay-only case is IPv4.
The existing coordinator-observed IPv4/IPv6 Direct cases remain green.

The withheld-1-RTT fixture now includes relay-only observation timeout and Stop cases.
Real ordinary TLS completes; observation frames cannot reach the helper. The operation
returns `route-observation-timeout` or `peer-cancelled`, with no offer/target access and
with the helper bind released. Its remote fixture does not serve allocations: success
requires failing at observation acquisition, not proceeding to allocation.

### Family compatibility and fallback

- A real IPv4 helper offer consumed with an explicit IPv6 loopback phone bind previously
  ended as `route-unavailable` after the attempted connection produced no response.
  The red test caught this. Both mismatch directions now return `route-family`; target
  remains untouched and the local bind is reusable.
- Helper preparation with a valid coordinator but wrong-family relay previously sent
  a real coordinator Initial before rejecting the later route. The red test caught it.
  Wrong-family coordinator and relay configurations now fail before bootstrap.
- Translated-address Direct HTTP still succeeds when an unused coordinated target hint
  is valid but in the other family. The real codec preserves that hint.
- Another test starts with a real unanswered Direct attempt, then reaches incompatible
  coordinated-target / relay hints. Both return `route-family`; the queued real Initial
  proves Direct was attempted first. Losing socket cleanup, uncancelled parent token
  and absence of target access are checked.

Generated-only mutations were caught and restored:

- Ignoring the coordinated target's family allowed selection to continue and eventually
  return a real connected session instead of the required local-configuration error.
- Ignoring the relay contact's family changed the result to upstream `peer-connect`,
  demonstrating that the adapter check, not a generic final failure, is covered.

The relay-hint mutation uses the public wire seam with the actual helper pin/grant left
unchanged in zeroizing buffers. It neither manufactures a grant nor accesses private
helper state.

Logs: `/tmp/wpt-family-{relay-red,relay-green,bind-red,bind-green,helper-red,candidates-green,selected-green,coordinate-mutation,relay-mutation,full,helper}.log`.

## Limits and next decisions

This policy intentionally fails closed rather than pretending to implement automatic
family selection. It does not establish public reachability, identify usable OS routes,
solve destination-dependent NAT, or guarantee mapping freshness while a QR is displayed.
An observed address may legitimately differ from the helper's local family through
translation; it is not rejected merely for that difference. The phone still needs a
usable local path to the selected contact and must authenticate the actual computer.

Before adding automatic dual-stack behavior, agree an interface for explicit local
bindings/capabilities per family and a policy for known-unusable routes. A pair of binds
alone would not solve mixed-family coordination on one UDP endpoint. Do not smuggle such
selection in as `Ok(None)` or rely on platform-specific mapped-address behavior.

NAT64/DNS64 or OS-provided IPv4 translation needs platform/network facts and real evidence.
Do not invent a prefix, rewrite numeric hints speculatively, or infer cellular capability
from these loopback tests. Android integration remains deferred.

The [final relay Stop follow-up](saorsa-route-negative-matrix.md#follow-up-stop-during-the-phones-final-relay-dial)
now covers response-wait/partial-TLS cancellation (117 routing tests plus a fresh helper
process suite). The [observer safety follow-up](saorsa-route-negative-matrix.md#follow-up-observer-control-loss-and-ineligible-reports)
adds explicit control closure and authenticated ineligible-report coverage (118 tests and
a fresh helper process suite). The approved [virtual-admission extension](saorsa-virtual-admission.md)
reached 122 tests, covering pre-TLS bounds, expiry/HTTP recovery and hook isolation.
The [mixed-ingress follow-up](saorsa-virtual-admission.md#follow-up-repeated-mixed-ingress-waves-and-drop)
reached 124 with repeated expiry/recovery and drop under saturation. The approved
[constructor-time helper hook](saorsa-preparation-admission.md) reaches 130 with
preparation refusal, cancellation, registry lifecycle and real observer provenance.
The [phone phase gate](saorsa-phone-ingress.md) reaches 137 without further transport
changes. The [authorization/Stop checks](saorsa-authorization-stop.md) reach 144 tests.
Next safety work: malformed/replayed frames, further authorization interleavings,
generic default-constructor ingress, transport buffering and sustained overload. Then
seek explicit approval for bounded synthetic probes through existing public participants.
Existing review patches still need production promotion; public cellular-to-Wi-Fi behavior
and Android migration remain unproven.
