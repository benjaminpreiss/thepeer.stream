# Can pinned upstream Saorsa relay authenticated peer traffic?

Follow-up: [bounded public participant proof](saorsa-public-participant-probe.md)
now records two successful public single-carrier data trials, six hostile-response
tests and five local relay cases. The original three-case investigation below is
retained as historical source/evidence context.

## Answer

**Yes, the core capability works in a local proof.** Upstream can select an
already-connected peer as a relay, establish a separate inner QUIC/TLS connection
to the computer through it, and carry bidirectional application bytes. It also
works when the computer publishes its own relay allocation and both endpoints
use outgoing relay carriers. No custom paired-session runtime was used for this
proof.

This is not a claim that public relays were discovered or contacted, that cellular
NAT traversal works, that the current APK uses this path, or that stock defaults
meet all our security/resource policies. Existing relay/provisioning sources and
Android staging were preserved.

## Actual tests

Run from the repository root:

```sh
bash experiments/direct-traversal-review/verify-upstream-relay.sh
```

Three real loopback-network tests passed against pinned Saorsa revision
`e2f08388088313c82cac393afa6ca27b9ce00516`:

1. Invoke the upstream relay dial leg and exchange a bounded public byte fixture.
2. Seed a normal phone-to-relay peer connection, leave both configured relay lists
   empty, invoke `connect_with_fallback`, and observe `ConnectionMethod::Relayed`
   naming that connected peer. Exchange bytes through the resulting connection.
3. First call the computer's upstream `setup_proactive_relay`, use the allocated
   address as its contact, then repeat connected-peer relay selection and data
   exchange. Both phone and computer application connections use virtual sockets;
   the computer's contact is distinct from its original physical socket.

In every case:

- The phone's returned peer identity and actual inner TLS certificate match the
  computer, not the relay. Computer and relay have distinct generated identities.
- Our existing `PinnedVerifier` accepts the actual computer certificate with its
  expected pin and rejects it with the relay's pin, before sending fixture bytes.
- Data travels on a bidirectional stream of **that same checked connection handle**,
  avoiding an address-based relookup between pin validation and writing.
- The computer sees the relay allocation as its peer, not the phone's original
  socket; the phone's application connection is virtual, not a direct UDP success.
- The public fixture round-trips exactly and connection closure reaches the other
  endpoint. All three endpoint owners are shut down after ordinary test failure
  or success.

### Deliberate test seams / limitations

The runner uses the existing pinned-source preparation recipe and its two reviewed
patches, then modifies **only the generated copy**:

- Expose `try_relay_connection` for deterministic testing of the otherwise private
  relay leg; its implementation body is unchanged.
- Empty only the initial direct-attempt address list in the fallback routine.
  This deterministically forces the failure path without racing a zero-duration
  timeout against fast loopback handshakes. Candidate selection, relay establishment,
  TLS, forwarding and application data handling are not replaced. Hole punching
  is disabled for these relay tests.
- Replace the two internal relay-server constructor calls with the reviewed
  `new_local` constructor so allocations remain loopback-only and UPnP stays off.
  The stock constructor would use wildcard allocation sockets; no such listeners
  were started by this proof.

All peers run in one host test process, with real sockets and TLS. There is no NAT
emulator, packet-capture/relay-memory inspection, real app grant, HTTP/browser
fixture, Android execution or public-network trial. The byte fixture is public.
This demonstrates the encrypted transport/identity seam, **not** full application
integration or a complete security audit. It is not an unmodified stock-daemon test.

## Source trace

All immutable links refer to the pinned revision above.

### Relay selection is local candidate selection, not a global directory

`connect_with_fallback_inner` uses the caller's relay list; if empty it takes the
first configured NAT relay, or otherwise connected peers other than the target.
The second test exercises the connected-peer branch, not discovery of an unknown
Internet node. A candidate can still reject relaying or be unreachable.

[Candidate selection](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/p2p_endpoint.rs#L2103-L2181).

### There really is an inner peer TLS connection

`try_relay_connection` establishes an outer CONNECT-UDP Bind session, creates a
`MasqueRelaySocket`, clones the peer TLS client config onto a new abstract-socket
endpoint, then calls `relay_endpoint.connect(target, "peer")`. It extracts the
**inner connection's** identity and stores that connection. The relay carries
inner QUIC packets, rather than terminating this peer application TLS session.

[Relay dial implementation](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/p2p_endpoint.rs#L2728-L2886),
[carrier/control setup](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/nat_traversal_api.rs#L4741-L4873).

For the destination behind a NAT, merely relaying the phone's outgoing packets
would not make the computer's private address reachable. Upstream additionally
provides proactive inbound relay allocations. The third test exercises this
mechanism locally. Production should evaluate the prepare → canary → publish APIs,
not blindly advertise an untested allocation.

[Proactive relay API](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/p2p_endpoint.rs#L3438-L3496).

### PQ signature verification is not the application's authorization policy

Endpoint setup configures raw public keys with `allow_any_key()`. The verifier
accepts an arbitrary well-formed ML-DSA key at the certificate-selection step but
its TLS 1.3 signature callback executes real ML-DSA verification. That authenticates
possession of a key, not that the holder is the computer chosen by the user.

[Endpoint TLS setup](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/nat_traversal_api.rs#L3731-L3841),
[RPK verifier](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/crypto/raw_public_keys.rs#L99-L180),
[builder](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/crypto/raw_public_keys.rs#L568-L645).

Do not authorize app access from `PeerConnection.authenticated` alone. The wrapper
must check the actual connection's canonical computer pin before sending a fresh
app grant and keep using that connection. Our real verifier was reused in the tests;
no assertion-only certificate verifier or fake crypto provider was introduced.

## Crypto failure-path finding (separate offline tests)

```sh
bash experiments/direct-traversal-review/verify.sh
```

This now passes **six offline tests**: the original four strategy/configuration
cases plus two crypto-provider characterizations:

- Normal `configured_provider_with_pqc(None)` produces a nonempty list containing
  only PQ key-exchange groups.
- Passing a deliberately invalid configuration with both PQ algorithms disabled
  makes `create_crypto_provider` fail. The higher-level
  `configured_provider_with_pqc` catches that error and returns
  `configured_provider()`, which uses the AWS-LC default provider. The returned
  provider contains non-PQ key-exchange groups. This branch also sets the debug
  KEM-only flag, so that flag must not be treated as enforcement.

[Wrapper and fallback](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/crypto/rustls.rs#L687-L722),
[provider creation errors](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/crypto/pqc/pqc_crypto_provider.rs#L111-L168).

**Qualification:** this is an error-path/provider-construction result, not a
classical TLS negotiation, remote downgrade exploit, or evidence that normally
configured/deployed nodes use classical key exchange. The P2P config builder has
its own validation; this test deliberately calls the lower-level provider wrapper.
It does show why we must preserve our application's fail-closed provider checks
rather than rely only on a “PQC enabled” label. The existing application calls the
fallible factory and rejects providers containing non-PQ groups; that code was not
changed by these experiments.

## Remaining integration/security work

**Updated scope:** the user chose upstream crypto policy without independent PQ-only
enforcement and will consume existing network relays, not operate one. Thus strict
provider-error enforcement and relay-operator controls below are historical review
recommendations, not current product prerequisites. Client-side resource bounds,
computer pin/grant authorization and application integration remain required.

- Reuse this upstream transport instead of reimplementing it, but retain our
  application grant, fixed localhost target, request guards, fresh browser origin
  and foreground-owner/Stop behavior. None of that is supplied by a stock P2P node.
- Adapt the application session to an already-established checked peer connection.
  Audit ALPN, stream consumers, peer pruning, cancellation and endpoint ownership.
  A P2P unidirectional reader already exists; relay-enabled endpoints also consume
  bidi streams for CONNECT-UDP. Do not introduce competing readers.
- Ensure PQ provider failure stays a hard failure in the integrated path, including
  configuration validation and the error branch described above.
- Preserve our explicit no-resumption/no-0-RTT requirements; the upstream TLS
  builder returns its base config without applying our application's overrides.
  Do not assume equivalent defaults merely because PQ signatures are enabled.
- Bound the upstream relay control response before allocation. The inspected
  `establish_owned_relay_session` reads a peer-supplied 32-bit response length and
  constructs `vec![0u8; resp_len]` without a local size check in that block. This is
  a source finding, not an attempted memory-exhaustion exploit. Inner encryption
  does not protect against resource abuse by a carrier peer.
- Retain the policy decision about where relaying is allowed. Upstream relaying
  does not automatically supply our paired-only routing, one-use pairing tickets,
  fixed bandwidth shares or aggregate-only metrics. Public node operation needs
  explicit abuse/resource controls, not only a successful crypto/data test.
- This investigation's loopback proof did not establish public availability or
  admission. A subsequent [bounded public participant probe](saorsa-public-participant-probe.md)
  records separate evidence; neither implies universal participant availability.

**Conclusion:** a Saorsa-based node is a credible common transport foundation for
coordination and optional relaying. The local results justify moving toward that
integration; they do not justify deleting the existing working relay or shipping
stock public-node defaults unchanged.
