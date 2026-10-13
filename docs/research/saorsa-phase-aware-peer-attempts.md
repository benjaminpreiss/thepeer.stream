# Phase-aware application connection attempts

Original investigation: 2026-10-11. Retained for the outgoing-attempt ownership,
no-response classification and authentication-before-grant contract now used by
Direct/Relay. Paths, commands and test totals below describe the investigation;
use [current build instructions](../BUILDING.md) for today's implementation.

## Native interface

`apps/android-controller/native/src/peer_connect.rs`, gated by `peer-client`, adds:

```rust
peer_connect::open(&endpoint, address, computer_pin, &grant, &stop).await
```

The result has three meanings, without parsing upstream error strings:

- `Ok(Some(session))`: full TLS completed; the actual peer's computer pin and
  application grant were checked using `Session::from_peer`. The exact connection
  handle is retained. No connection-table relookup or 0-RTT app data is used.
- `Ok(None)`: the initial response deadline expired, and atomic abort found **no
  recorded QUIC peer response and no recorded error**. The caller may consider the
  next path. This is not proof that the remote host is absent or that no packet
  remains in flight/in an OS queue.
- `Err`: terminal, including connection setup errors, TLS/protocol/authentication
  rejection, post-response timeout, wrong pin, rejected grant, malformed grant or
  cancellation. The caller must not turn these errors into route fallback.

Setup budgets are three seconds for the first response, ten seconds for completing
TLS, then the existing ten-second application authorization budget. These budgets
end when setup completes; they impose no new session lifetime or stream-count ceiling.
Malformed grants are rejected before dialing, with bounded canonical 64-character
lowercase-hex validation. Full offer/candidate validation still belongs to the future
versioned offer decoder.

The endpoint's cryptographic configuration is reused, not rebuilt or independently
restricted. Callers must construct application-owned, client-only endpoints with relay
serving, UPnP and unwanted advertisement disabled, as the host fixture does.

## Reviewed upstream interface extension

`experiments/direct-traversal-review/0004-cancellable-peer-attempt.patch` applies to
exact transport revision `e2f08388088313c82cac393afa6ca27b9ce00516` (0.37.0).

SHA-256:

```text
f68612aed60d8df84ceeb3bba85a14b06cc145d1596567cb335d3a4183217dba
```

It adds:

1. `P2pEndpoint::begin_connect`: start an **unregistered**, caller-owned connection
   with the endpoint's existing crypto configuration. No automatic fallback, P2P
   application reader or connection-table relookup is involved.
2. `Connecting::abort`: under the connection state lock, capture any recorded error
   or peer-response flag, then close that attempt. This avoids separately checking
   response state and cancelling across a race. A recorded error is not reduced to
   a "no response" boolean. A QUIC response is not proof of TLS peer identity.
3. Public visibility for the existing `accept_with_connection` method, without a
   body change.

The new `prepare-peer-review.sh` verifies that hash before applying it on top of the
existing review preparation (including the 4096-byte control-response bound). The
pinned checkout/cache is untouched. Legacy constructors/fallback algorithms, wire
encodings and cryptographic policy are unchanged. The patch carries upstream context;
see the existing [upstream MIT notice](../../patches/saorsa-transport/UPSTREAM-LICENSE-MIT).

**These extra patches are still not in production `prepare-native.sh`.** Only the
review runner enables `peer-client`; the Android build retains its existing feature
selection. Do not enable this feature against an unprepared stock dependency.

## Ownership and Stop

The attempt owns its in-progress `Connecting`. Cancellation or dropping the setup
future releases it; a successful `Session` owns and closes its checked connection.
Neither operation closes unrelated coordinator/control connections on the shared
endpoint. Cancellation already signalled, or dropping an unpolled `open` future,
starts no datagram traffic.

Important: upstream shutdown closes its registered connection table, not these
unregistered application-owned connections. **Stop/drop attempts and sessions first,
then shut down their endpoint.** An initial test incorrectly assumed endpoint shutdown
alone owned an unregistered attempt; it timed out and exposed this ownership contract.
The interface documentation now makes that order explicit. After setup succeeds,
callers must also wire Stop into `application::drive`; the setup token is not a hidden
session-lifetime task.

## Twelve real loopback regressions

`peer_connect_tests.rs` covers:

1. Successful scoped authorization and peer close on session drop.
2. A silent UDP target returns the fallback-permitting result within its deadline.
3. One real server datagram followed by a stalled TLS handshake is a terminal timeout.
4. Stop cancels an in-flight attempt while a separate control connection stays usable.
5. The expected public pin with a **wrong TLS signing key** fails terminally. A positive
   control uses the same P2P configuration with matching keys and authorizes successfully.
6. Malformed grants cause no network attempt.
7. Wrong computer pin opens no grant-bearing application stream.
8. Rejected grant is terminal and never opens the target.
9. Atomic abort after a response cannot report silence and closes the peer.
10. Atomic abort preserves a previously recorded TLS negotiation error.
11. Stop while awaiting application authorization closes the checked peer.
12. Already-cancelled and unpolled attempts send no datagrams.

The stalled-handshake test injects loss at a real loopback UDP forwarding socket;
it does not fabricate a TLS result or mock internal collaborators. The first
application success, silence deadline, post-response deadline, cancellation and
malformed-input tests were observed failing before their implementations, then passing.

The forged-signature test was strengthened with a positive control. That caught an
initial fixture with mismatched certificate-type/transport configuration. The final
fixture uses actual `P2pEndpoint` instances, changing only whether the supplied public
and private keys match. No crypto check was weakened to make the fixture pass.
Earlier coordinator notes calling their incompatible native/P2P fixture specifically
an "ALPN rejection" have likewise been narrowed to "TLS negotiation rejection".

## Application and public evidence

The existing single-carrier application probe now uses `peer_connect::open` for its
phone-to-computer application dial. Helper acceptance runs concurrently so it can
complete the real grant exchange. The computer's accepted handle is still checked
as virtual, and the route is labelled **single-carrier relay**, not direct merely
because the initiator uses an ordinary UDP socket.

Both local application paths passed: the new phase-aware single-carrier path and the
existing coordinated nonvirtual path. One explicit public synthetic-fixture trial
through the first bundled participant also passed:

```text
public-application=ok route=single-carrier-relay computer-pin=checked grant=authorized http=200 stop=closed cleanup=completed
```

This verifies the new outgoing-attempt implementation through actual participant
traffic, application authorization, HTTP/CSP/body checks, Stop and local cleanup.
Only the ephemeral synthetic localhost fixture was exposed. The grant stayed in
zeroizing in-memory storage/serialized buffers and never entered logs, argv, files
or Intents. Both logical roles were on this host. Remote cleanup accounting was not
independently observed. No new public punching trial was run, and its previous
timeout is not claimed fixed.

Final validation:

- **74 native/peer tests passed**: 62 existing native tests plus the twelve above.
- **96 legacy patched relay-review tests passed**; the working paired-relay path remains.
- Both local application-path tests passed; the new public relay application trial passed.
- Exact production-source **host JNI** ABI/lifecycle check passed (not Android).
- Four native-staging validation tests passed; no artifact was staged. Rust formatting,
  shell syntax, public opt-in rejection and diff whitespace checks passed.

```sh
# Loopback only; CI runs these without public options.
bash experiments/direct-traversal-review/verify-peer-connect.sh
bash experiments/direct-traversal-review/verify-application-peer.sh --local

# Explicit public traffic, never CI.
bash experiments/direct-traversal-review/verify-application-peer.sh --public
```

Logs include `/tmp/wpt-phase-final-twelve.log`, `/tmp/wpt-phase-final-full.log`,
`/tmp/wpt-phase-signature-matched-control.log`, `/tmp/wpt-phase-application-local.log`,
`/tmp/wpt-phase-application-public.log`, `/tmp/wpt-phase-legacy-relay.log` and
`/tmp/wpt-phase-production-jni.log`.

## Remaining integration

- Follow-up: [adapter-owned coordination](saorsa-handshake-hook-adapter.md) now
  covers incoming/outgoing punching through a generic removable transport hook.
  The one-attempt policy stays in our adapter; integrate its explicit ownership
  contract into the complete helper/route lifecycle.
- Implement the bounded route selector, preserving provenance and terminal errors.
- Add the versioned candidate-bearing offer/helper lifecycle, independent of relay
  availability; do not reinterpret `wpr1:`.
- Review/promote required patches and all source/staging provenance inputs together,
  then wire Android/JNI/foreground ownership and operator-test cellular versus Wi-Fi.

Native source changed, but no Android build, staging, installation, dependency upgrade,
public-relay hosting, provisioning change, commit or release occurred. The installed
APK still does not use this path. Broader hostile-peer resource auditing remains open.
