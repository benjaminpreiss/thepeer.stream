# Public Saorsa participant: bounded client and live relay proof

Follow-up: [real application integration](saorsa-peer-application-integration.md)
subsequently passed fresh grant authorization, HTTP and Stop through this participant.
The byte-only probe described below remains a separate evidence layer.

Date: 2026-10-10. This follows the user's approval to continue from source review
into bounded public compatibility testing. It supersedes “no public probe yet” for
this experiment only, not earlier historical evidence.

## Result

**A published Autonomi/Saorsa participant accepted our pinned transport and carried
an independently authenticated inner peer connection with bidirectional data.**
The successful single-carrier trial was repeated successfully. No relay was deployed
or served by us, and no dependency was upgraded.

Both logical endpoints ran on this computer. The remote participant was the first
native seed in the frozen official
[client bootstrap resource](https://github.com/WithAutonomi/ant-client/blob/9a2b6353a88207dd4d6ed99a5158c370531a7e09/ant-core/resources/bootstrap_peers.toml).
Only that participant was contacted; the initial maximum of two candidates did not
cause a second dial because the first completed its handshake.

Successful path:

```text
phone-role QUIC socket → participant's public UDP allocation
                         → computer's outgoing MASQUE carrier → computer-role
       [inner phone↔computer QUIC/TLS spans the entire path]
```

The phone role does not need its own MASQUE carrier for this path. A socket dial to
a computer's relay contact is still a **relayed application route**, even if an
upstream connection-method label calls that socket dial “Direct”. Route reporting
must account for the candidate's provenance.

Observed twice:

```text
stage=quic-handshake result=ok
stage=address-observation available=true
stage=computer-relay-allocation result=ok ipv4=true
stage=computer-pin result=ok
stage=accepted-handle lookup-available=false mapped-address=true
stage=public-byte-roundtrip result=ok computer-carrier=true close-observed=true
stage=cleanup result=ok
```

The computer's actual inner TLS certificate passed our existing pin verifier before
any fixture bytes. A bounded public fixture round-tripped exactly. The receiver's
connection used a virtual relay socket; the initiator used its ordinary UDP socket.
Peer close was observed. Allocation abort and both local endpoint shutdown APIs
completed. Remote capacity/accounting release was not independently inspected.

## Client response hardening

New review-only patch:
`experiments/direct-traversal-review/0003-client-relay-response-bound.patch`

SHA256: `6c55a783245e3045d9cf852bfa5836fb0b15528542da6d903c0b3f14f038d04d`.

It rejects zero or >4096-byte CONNECT-UDP control responses **before allocating**
the body. The existing upstream 10-second handshake deadline is unchanged and
covers both partial prefixes and bodies. This modifies only isolated prepared
sources; it is not yet in the production Android preparation recipe.

Six real loopback TLS tests passed: zero, 4097, maximum-u32 prefixes; exactly 4096
bytes reaching the decoder; stalled prefix; stalled body. The oversized cases do
not provide a body and must reject before the handshake timeout. The max-u32 case
is run only with the verified bound patch applied, not against the original allocator.
This is one specific client hardening fix, not a complete hostile-relay audit.

## Diagnostic findings and limits

### Two-carrier Internet path remains unresolved

The first public run requested a computer allocation and used upstream's explicit
phone-side relay dialer too. The inner peer handshake timed out. This is NOT a
successful public two-carrier proof, nor evidence that all relays are unavailable.

A new local test confirmed an unpublished prepared allocation can already receive
inner peer traffic, ruling out a general requirement to publish before dialing.
Switching only the public phone-side path to a normal dial of the allocated contact
established the inner peer TLS session. This narrows the unresolved issue to the
two-carrier/public-path scenario; it does not establish its root cause.

### Address relookup loses the accepted connection

The first single-carrier run passed the computer pin, then failed to retrieve the
computer's accepted connection by address. Subsequent runs retained the actual
accepted handle and demonstrated both:

- the accepted source was an IPv4-mapped address;
- `get_quic_connection(source)` returned `None`, while the accepted handle carried
  data successfully.

Pinned source explains the mismatch: `accept_with_connection` inserts the raw
remote address into `connected_peers`, whereas `get_quic_connection` canonicalizes
before looking up that map. There are also lifecycle races inherent in relookup.
See [lookup](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/p2p_endpoint.rs#L1550-L1569)
and [accepted handle/map insertion](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/p2p_endpoint.rs#L2893-L2973).

The probe uses the existing `accept_with_connection` implementation rather than
relooking up a connection. That method is crate-private upstream, so the review
runner exposes it in the generated copy. This is an integration seam to review,
not a shipped upstream normalization fix. A fifth local test covers the single-carrier
flow with the retained handle; the mapped-address failure itself was observed in
the public test, not recreated by that loopback case.

## Safety, provenance and reproducibility

```sh
# Loopback only; these run in CI.
bash experiments/direct-traversal-review/verify-client-bounds.sh
bash experiments/direct-traversal-review/verify-upstream-relay.sh

# Explicit public traffic; NEVER run automatically in CI.
bash experiments/direct-traversal-review/probe-network.sh --public 1 single-carrier
```

- Six bounds/deadline cases and five relay/data/identity cases passed locally.
- Public test uses pinned `e2f08388088313c82cac393afa6ca27b9ce00516` (0.37.0), existing
  reviewed patches 0001/0002, the experimental response bound and visibility-only
  changes for two existing methods. No crypto policy or relay algorithm was replaced.
- Public runner does not inject the local test's forced-direct-failure seam.
- Relay serving, UPnP and address advertising are disabled on our endpoints.
  No DHT API calls or storage/payment operations are performed. Remote participants
  may naturally record an ordinary peer connection; this is not a claim of anonymity.
- At most four connections per endpoint; bounded operation/abort/shutdown waits;
  public fixture reads capped at 1024 bytes. Only officially listed seeds are used,
  and allocation IP must match the selected seed. Errors after a successful seed
  handshake are terminal, including pin errors.
- No live app grant, localhost application target, browser, Android build, scanner,
  packet capture, VPS provisioning, firewall change or production staging.

Logs from this session: `/tmp/wpt-client-bounds.log`,
`/tmp/wpt-upstream-five-local.log`, `/tmp/wpt-network-participant-probe.log`,
`/tmp/wpt-network-single-carrier.log`, `/tmp/wpt-network-authoritative-handle.log`,
`/tmp/wpt-network-authoritative-repeat.log`.

## What this does not prove / next work

This is real public relay interoperability and data evidence, **not** cellular-to-Wi-Fi
application evidence, automatic DHT discovery, coordinated hole punching, healthy
long-session behavior, all-node availability or the deployed node's precise version.

Next: integrate retained peer connections with the application protocol and lifecycle;
prepare identity/grant independently of relay success; add versioned candidates and
approved direct → hole punch → network relay selection. Keep authentication failures
terminal and resource bounds on the client. Then operator-build Android and test
phone cellular versus computer Wi-Fi. The existing APK/helper remain unchanged.
