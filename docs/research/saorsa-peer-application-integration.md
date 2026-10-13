# Application forwarding over an existing Saorsa participant

Date: 2026-10-10. **Real application authorization and HTTP forwarding passed through
the first officially bundled public participant**, using our pinned transport and
review-only client response bound. Both logical roles still ran on this host.

This advances the earlier [public byte-only proof](saorsa-public-participant-probe.md).
It does not add route selection, QR candidates or Android activation.

[Phase-aware attempt follow-up](saorsa-phase-aware-peer-attempts.md): the single-carrier
probe now uses the native outgoing attempt module and has passed again publicly.
The retained-connection seam is now used by the current Direct/Relay implementation.
The paths, commands and counts below record the original investigation, not current
build instructions. See [current source/build guidance](../BUILDING.md).

## Implementation seam

`apps/android-controller/native/src/application.rs` now supports:

- `Session::from_peer(connection, computer_pin, grant)`: accepts an already established
  upstream connection, checks its actual canonical TLS identity using our existing
  pin verifier, then runs the same application-grant exchange as the existing path.
  No application stream is opened before pin verification.
- `serve_peer(accepted_connection, inspector, target_port, counts)`: serves the same
  one-use grant and fixed-localhost-target protocol on the authoritative accepted
  handle. No address-based relookup is required.

The caller retains the upstream endpoint and must shut it down separately. The
application session closes only its owned peer connection, not a shared endpoint
containing coordinator/relay control connections. The original LAN/paired-relay
constructors still retain and close their owned endpoint.

A connection guard is acquired **synchronously when either setup function is called**.
Dropping even an unpolled future therefore closes its connection. Guards also cover
pin/authentication failure, cancellation while waiting for an ack and session drop.
Application forwarding, request/origin guards, CSP, cookie handling, SSE/WebSocket
support, byte/concurrency bounds and healthy-session lifetime behavior remain in
the existing shared implementation rather than a second proxy implementation.

For upstream endpoints, relay serving/other application bidirectional-stream readers
must remain disabled. Upstream's unidirectional reader is separate from our application
bidirectional streams. The code inherits upstream connection cryptography; it does
not impose a new PQ-provider policy.

## Local evidence

Seven new native regressions exercise the existing HTTP/target and retained-connection
interfaces with real upstream TLS sockets:

1. HTTP 200 and CSP through the browser-facing listener, then Stop closes that listener.
2. Wrong computer pin closes without opening any application bidirectional stream.
3. Wrong fresh grant never opens the target; helper authorization fails.
4. Client setup cancelled before first poll closes its connection.
5. Helper setup cancelled before first poll closes its accepted connection.
6. Client cancellation while awaiting the grant ack closes the connection.
7. Dropping an authorized session closes it but leaves a separate coordinator connection
   on the shared endpoint usable for data.

The HTTP test was observed failing with the unimplemented seam, then passing.
Both unpolled-cancellation tests were observed failing with async-body-only guards,
then passing after synchronous ownership acquisition.

Validation:

- **62 baseline native tests passed**, including existing HTTP, SSE, WebSocket,
  wrong-pin/grant, 520-request and long-lifetime/cancellation coverage.
- **96 patched relay-review tests passed**, retaining the working paired-relay path.
- Exact production-source **host JNI** check passed: existing ABI, reservation/Stop
  lifecycle and sanitized validation. This is not an Android build/device test.
- A separate loopback single-carrier proof passed with real app grant, HTTP and Stop.

## Public application trial

```sh
# Safe for CI: local relay fixture only.
bash experiments/direct-traversal-review/verify-application-peer.sh --local

# Explicit real public traffic; never run automatically in CI.
bash experiments/direct-traversal-review/verify-application-peer.sh --public
```

Observed public result:

```text
public-application=ok computer-pin=checked grant=authorized http=200 stop=closed cleanup=completed
```

The probe:

1. Creates a synthetic TCP HTTP fixture bound only to localhost.
2. Prepares the computer's identity and a fresh real 256-bit app grant in memory,
   before attempting relay allocation.
3. Connects to the first native seed from the frozen official
   [bootstrap resource](https://github.com/WithAutonomi/ant-client/blob/9a2b6353a88207dd4d6ed99a5158c370531a7e09/ant-core/resources/bootstrap_peers.toml).
4. Prepares the computer's outgoing MASQUE allocation; validates the allocated IP
   against the selected seed; dials that public contact from the phone role.
5. Retains the actual accepted connection, checks the computer pin through
   `Session::from_peer`, and authorizes the scoped grant through `serve_peer`.
6. Only then binds the browser-facing loopback listener with a fresh `wpt-<hex>.localhost`
   origin; sends an HTTP GET through `application::drive`; verifies HTTP 200, CSP and
   the exact public fixture body.
7. Stops the gateway, verifies helper authorization and exactly one forwarded request,
   confirms the listener is closed, aborts the allocation and shuts down both owners.

Unlike the earlier byte-only probe, this uses a real **synthetic-fixture grant** and
localhost target. No existing user application was exposed. The grant is never
printed, put in argv/Intents/files, or sent before computer-pin verification; caller
storage and serialized authorization bytes use zeroizing buffers. The inspector
retains only its grant hash. Errors are static stage labels.

The host runner exposes upstream's existing crate-private `accept_with_connection`
without changing its implementation. It applies the experimental 4096-byte control
response bound. For local fixtures only, relay allocations are constrained to loopback.
Public endpoints do not serve relays, advertise addresses or enable UPnP. No DHT,
storage/payment or deployment operation is performed. The 45-second operation bound
and bounded abort/shutdown waits are diagnostic limits, not application lifetimes.
Remote relay accounting cleanup was not independently inspected.

Logs: `/tmp/wpt-peer-app-red.log`, `/tmp/wpt-peer-cancel-red.log`,
`/tmp/wpt-peer-helper-red.log`, `/tmp/wpt-peer-full-native.log`,
`/tmp/wpt-peer-app-relay-local.log`, `/tmp/wpt-peer-app-public.log`,
`/tmp/wpt-peer-app-relay-regressions.log`, `/tmp/wpt-peer-production-jni.log`.

## Remaining work

- Implement the approved direct → coordinated hole punching → network-relay route
  selection and candidate provenance. A dial to an allocated relay contact must not
  be presented as a direct computer path merely because the initiator uses UDP.
- Prepare a versioned offer and independent helper lifecycle. This proof prepares
  identity/grant before allocation, but still deliberately forces a relay path;
  it is not the new pairing/helper implementation.
- Review/promote the client bound and accepted-handle interface for production builds;
  the experimental source recipe is not the APK recipe.
- Wire Android/JNI/foreground ownership and scanning without capabilities in Intents,
  preserving explicit Connect, browser launch, fresh origin and Stop.
- Operator-test cellular phone versus Wi-Fi computer, including actual direct/punch
  attempts, relay fallback, browser/SSE/WebSocket and cancellation. The public trial
  here was one host HTTP GET, not a phone/browser or full streaming fixture trial.

The public two-carrier handshake timeout from the prior probe remains unresolved.
No dependency upgrade, Android build/staging/install, public relay hosting or
provisioning change occurred. Native source changed; the existing APK still lacks
this network route, and a future operator build must produce a matching source receipt.
