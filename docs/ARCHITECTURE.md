# Architecture

[Overview and installation](../README.md) · [Build from source](BUILDING.md) ·
[Validation status](STATUS.md)

**The short version:** thepeer puts an authenticated, encrypted connection between
a phone-side HTTP gateway and a computer-side localhost forwarder. Saorsa provides
the underlying peer transport, cryptography and optional relay mechanism. Thepeer
provides pairing, route policy, application authorization, browser integration and
session ownership.

This describes the current Android-browser MVP, not a proposed VPN, public hosting
service or terminal-to-terminal tunnel. Hosted release/signing and post-refactor
physical validation remain separate gates; see [release readiness](RELEASE-READINESS.md).

## 1. What runs where

```text
COMPUTER                                     ANDROID PHONE

Local web application                        Ordinary browser
localhost:3000                                http://thepeer-<nonce>.localhost:PORT
       ^                                               |
       | localhost TCP/HTTP                            | loopback HTTP
       v                                               v
+-----------------------------+              +-----------------------------+
| thepeer terminal executable |              | thepeer Android app         |
|                             |  QR pairing  |                             |
| CLI + terminal QR           | -----------> | UI + camera scanner         |
|                             |              | Foreground service          |
| Shared Rust core:           |              | JNI bridge to Rust          |
| - fixed-target forwarding   |              | Shared Rust core:           |
| - grant authorization       |              | - local HTTP gateway        |
| - CLI admission/cleanup     |              | - route/pin/grant handling  |
|                             |              | - connection/Stop ownership |
| Saorsa peer transport       | <==========> | Saorsa peer transport       |
+-----------------------------+              +-----------------------------+
                   Encrypted peer connection
                      Direct or via Relay
```

The browser does **not** run Saorsa, WebRTC or a tunnel service worker. It talks to
the app's local gateway. The local web application needs no Saorsa integration;
it receives ordinary localhost traffic from the CLI.

The remote leg is encrypted peer-to-peer. The browser-to-app and CLI-to-target
legs are local HTTP/TCP, not additional end-to-end TLS sessions.

## 2. Our components and their responsibilities

```text
crates/cli                           apps/android-controller
package: thepeer-cli                  Java UI, QR scanning, foreground service
binary: thepeer                                   |
       |                                         | JNI (in-process calls)
       |                             crates/tunnel-android
       |                             Native reservation and Stop ownership
       |                                         |
       +------------------+----------------------+
                          v
                crates/tunnel-core
                Shared application/routing logic
                          |
                          v
                saorsa-transport 0.37.0
                Exact revision + five reviewed local patches
                          |
                          v
                QUIC/TLS, UDP sockets, OS networking
```

These crates are libraries linked into the two applications, not separate daemons.
Both applications share the core; the core's `host` feature enables desktop-only
CLI functionality.

- **[CLI](../crates/cli/src/main.rs):** Parse target/participant options, select a
  local interface, prepare the CLI, display a live terminal QR and handle terminal Stop.
- **[Android app](../apps/android-controller/):** Connect/Stop UI, bounded camera
  scanning, in-memory handoff, foreground notification and browser launch. ZXing
  decodes QR symbols; our code validates the offer.
- **[Android Rust bridge](../crates/tunnel-android/src/routing_owner.rs):** JNI
  boundary, one native owner/reservation, cancellation and gateway lifetime.
  JNI is not a network protocol.
- **[Offer codec](../crates/tunnel-core/src/route_offer.rs):** Canonical `wpa2:`
  descriptor: computer key fingerprint, fresh 256-bit grant and routing hints.
  Reject malformed/retired formats without networking.
- **[Route planner](../crates/tunnel-core/src/route_plan.rs):** Decide which Direct
  hints are eligible, deduplicate them, then consider the separately advertised Relay contact.
- **[Route owner](../crates/tunnel-core/src/peer_route.rs):** Prepare endpoints/relay
  allocation, observe addresses, enforce admission, select a route and own its cleanup.
- **[Connection adapter](../crates/tunnel-core/src/peer_connect.rs):** Retain the actual
  connection attempt, distinguish silence from TLS/protocol failure and authorize the
  resulting session.
- **[Application gateway](../crates/tunnel-core/src/application.rs):** Verify the
  retained peer identity, exchange application authorization, proxy browser requests
  over QUIC streams and forward to the fixed target.
- **[HTTP policy](../crates/tunnel-core/src/http.rs):** Shared grants/framing and HTTP
  support; related modules implement host-side authorization and target handling.
- **[Runtime owner](../crates/tunnel-core/src/route_runtime.rs):** Own asynchronous
  transport work and drain it on cancellation/drop rather than leaving background
  endpoints alive.

## 3. Exactly what comes from Saorsa

We use **`saorsa-transport` 0.37.0**, pinned to
`e2f08388088313c82cac393afa6ca27b9ce00516`, with the reviewed patches below.
We do not run an Autonomi node or import an entire decentralized network stack.

- **Peer networking:** Saorsa supplies `P2pEndpoint`, UDP networking and QUIC
  connections/streams. Thepeer decides which endpoint to contact, when to stop and
  which failures may permit fallback.
- **Cryptography:** Saorsa supplies TLS 1.3 and the upstream post-quantum crypto
  implementation, including ML-DSA peer signatures. Thepeer identifies the intended
  computer by comparing the **actual retained connection's** public-key fingerprint
  with the QR pin before disclosing the grant.
- **Address observations:** Saorsa supplies connection-observed public socket
  addresses. Thepeer selects participants explicitly and treats observations as
  routing hints, not identities or reachability guarantees.
- **Relay transport:** Saorsa supplies MASQUE relay allocation, carrier transport
  and virtual ingress. Thepeer prepares the optional single-carrier Relay route,
  preserves separate relay-contact provenance and owns allocation/session cleanup.
- **Admission and lifecycle:** Saorsa supplies transport callbacks and connection
  handles, extended by our patches. Thepeer implements phone-side refusal, bounded
  CLI admission, application authorization and Stop semantics.

The route planner also adapts a conservative shared-WAN rule from **Saorsa-core**,
with its MIT notice retained. This is attributed application code, not a runtime
dependency on Saorsa-core's DHT or discovery system.

We **do not use upstream's catch-all `connect_with_fallback` as our routing policy**.
It cannot substitute for the rule that authentication/protocol failures are terminal.
Saorsa's transport authentication and our application grant are distinct:

```text
Saorsa TLS:       Did this peer prove possession of its cryptographic key?
QR pin check:     Is that key the intended computer's key?
Application grant:
                  Is this session authorized to access the fixed local target?

All three matter. A reachable address or valid TLS peer alone is not authorization.
```

### Why we carry five patches

- **0001:** Opt-in forwarding policy/limits and a local allocator.
- **0002:** Bounded established-stream socket and lifecycle controls.
- **0003:** Bound client relay-response collection.
- **0004:** Caller-owned cancellable attempts and retained accepted connections.
- **0005:** Constructor-installed handshake hooks, including virtual ingress.

0001/0002 originated in retired relay experiments; preserving their reviewed opt-in
interfaces does not mean thepeer operates a relay service. All five remain in the
build stack. These are local extensions, not claimed upstream-merged changes.
They do not replace upstream cryptography or require remote participants to adopt
a new wire protocol. Exact hashes, scope and limitations are in the
[patch documentation](../patches/saorsa-transport/README.md).

## 4. Pairing and connection sequence

### Optional services: two distinct roles

**An observer does not have to be a relay.** Observation reports addresses;
relaying forwards packets. The roles can be performed by the same service or by
separate services:

- **Neither option:** no observer or relay; local-only use needs no public service.
- **`--observer IP:PORT` only:** obtain address observations, but no Relay fallback.
- **`--relay IP:PORT` only:** use that service for both observation and Relay.
- **Both options:** use the explicit observer for routing observations and the
  configured relay for Relay allocation. They need not be the same service.

### Target resolution at CLI startup

On the computer, the CLI RESOLVES THE TARGET ONCE AT STARTUP, before publishing
the pairing QR.

```text
  --target localhost:3000
              │
              ▼
  ┌─────────────────┐  hostname  ┌──────────────────┐
  │ CLI             │ ─────────► │ Operating system │
  │                 │ ◄───────── │ hosts file / DNS │
  └────────┬────────┘ IP addresses└──────────────────┘
           │          in OS order
           ▼
     All loopback? ── no / empty ──► Reject
           │ yes
           ▼
  ┌───────────────────────────────────────────────┐
  │ Save addresses + port → finish setup → QR     │
  └───────────────────────────────────────────────┘
```

Explicit IP targets skip the OS lookup but still require loopback validation.
See [local HTTP target rules](CLI.md#local-http-target) for accepted syntax,
validation limits, deadlines and connection behavior.

### Preparation and address observation

The observer and relay below are separate **roles**, not necessarily separate servers.
Each exchange happens only when its role is configured. With `--relay`, allocation
happens before displaying the QR—even if Direct later succeeds. “Direct first”
refers to selecting the application traffic route, not delaying relay preparation.

```text
CLI                         Observer role                 Relay role
      |                           |                            |
      |-- what address am I? ---->|                            |
      |<-- observed IP:port ------|                            |
      |                           |                            |
      |-- request relay allocation --------------------------->|
      |<-- reserved relay IP:port + carrier connection --------|
      |
      | Prepare QR with computer key pin, grant and route hints
      | (include observer address and relay contact if available)
      v
   Phone scans QR
      |
      |-- what address am I? ---->|
      |<-- observed IP:port ------|
      | Close temporary observation connection
      | Plan eligible Direct routes, then Relay if available
```

### Peer authentication and application access

The connection below is between phone and computer, either directly or through
the separately prepared relay. The observer is not in the application data path.

```text
CLI                                                  Phone app
      |                                                   |
      |<=========== establish peer TLS connection =======>|
      |                                                   | check computer key
      |<-- application grant -----------------------------| only after pin check
      |-- authorization acknowledgement ----------------->|
      |                                                   | serve local gateway
      |                                                   | and open browser
      |<=========== application traffic on QUIC =========>|
```

An **observer** is a compatible Saorsa service that tells a connecting device,
“this is the IP address and UDP port I see your traffic coming from.” This helps
thepeer understand addresses behind a router or mobile network. The computer's
observed address becomes a Direct routing hint; the phone's observed IP helps decide
whether trying a local address makes sense, for example when both devices appear
to share the same public IP. Observation does not prove they can reach each other.

The CLI's `--observer IP:PORT` option selects this service explicitly. Otherwise,
a configured relay also serves as the observer. Its address is included in the QR
so the phone can obtain its own observation. The observer role only supplies address
information: it receives no application grant and does not forward web traffic.
A service acting as a relay performs that separate forwarding role too.

The relay service address, observed CLI address and allocated relay contact
have different roles and must not be interchanged.

The QR is an **out-of-band bearer capability**, not just a server URL. It contains
both the intended computer's pin and a fresh one-shot grant. Keep it out of logs,
files, clipboard, screenshots, preferences and Intents. Android transfers it through
memory; service Intents carry only a nonsecret generation identifier.

The phone's browser origin has a fresh 128-bit nonce:
`thepeer-<32 lowercase hex digits>.localhost`. This separates browser origins across
sessions; it is not a public DNS name or the tunnel's application grant.

## 5. Direct versus Relay

### Direct — one peer connection

```text
  Phone Saorsa endpoint <---- UDP / encrypted QUIC ----> Computer Saorsa endpoint
```

### Relay — same logical peer connection, different packet path

```text
  Phone Saorsa endpoint
           |
           | UDP packets carrying inner peer QUIC
           v
  Relay's allocated UDP contact
           |
           | MASQUE carrier established by the computer
           v
  Computer Saorsa endpoint (virtual ingress)

  Inner peer TLS still terminates at PHONE and COMPUTER, not at the relay.
```

The computer first opens a connection to the relay. The relay reserves an IP address
and port for reaching that computer, which the CLI includes in the QR. The phone
sends encrypted packets to that reserved address; the relay forwards them to the
computer over the connection the computer already opened. Replies travel back through
the relay in reverse.

That computer-to-relay connection is called the **MASQUE carrier**. We call this
**single-carrier** because only the computer opens one; the phone does not need a
second MASQUE connection. Traffic still passes through the relay, so this is Relay,
not Direct.

The tunnel's encryption runs between the phone and computer, not just to the relay.
The relay can see addresses, timing and traffic volume, and can drop packets, but
cannot read the application grant or web requests and responses.

### When to try another route—and when to stop

**No response permits fallback. Failed authentication does not.** The phone must
not hide a security or protocol failure by silently switching to another route.

```text
Check QR data and IPv4/IPv6 compatibility
                 |
If the QR names an observer (a service that reports your visible address):
ask what address it sees for the phone, then close that temporary connection
                 |
Try an eligible direct address for the computer
                 |
          +------+-------------------------+
          |                                |
Connection and                     No response arrives
authorization succeed              (confirmed by the transport)
          |                                |
Use Direct                         Cancel attempt and finish cleanup
                                           |
                              Try another eligible direct address;
                              if none remain, try the configured relay
                              (if no relay is available, stop)

At any stage: wrong computer key, rejected grant, TLS/protocol failure,
invalid input, observation failure, incompatible address family,
cleanup failure, or the user taps Stop
                 |
                 +----> End this connection attempt. Do not switch routes.
```

A peer that responds but fails to complete TLS is not simply “no response.”
Fallback is reserved for the transport's conservative silence classification.

Direct eligibility is contextual, not a blind attempt at every private IP. Shared
public observations can support a local-scope attempt, but CGNAT and segmentation
make them imperfect hints. Relay is available only if explicitly configured and
successfully prepared. No default relay, coordinated hole punching, automatic
discovery, NAT64 synthesis, route racing, live route upgrade or reconnect is present.
See [offer and candidate policy](ROUTING-OFFER.md) for exact rules.

## 6. How a browser request reaches the target

### Request and response flow

After peer authentication and grant authorization, our adapter presents a
bidirectional QUIC stream as a readable/writable socket to Hyper. Hyper parses and
serializes HTTP; Saorsa handles encrypted, reliable transport.

```text
PHONE
  Browser
     |
     | local HTTP request
     v
  +-- Phone HTTP gateway (our Rust code, built on Hyper) --+
  | Hyper parses the request from the loopback listener    |
  | Our code checks Host / Origin / fetch and HTTP limits  |
  | Our code opens a bidirectional QUIC stream             |
  | Hyper writes the request as HTTP/1.1 bytes into it     |
  +--------------------------------------------------------+
     |
  Saorsa / QUIC
     | Splits stream bytes into frames, encrypts them
     | and sends packets over UDP (Direct or Relay)
     v
COMPUTER
  Saorsa / QUIC
     | Decrypts, handles missing packets and restores
     | ordered stream bytes on the authorized connection
     v
  +-- CLI -------------------------------------------------+
  | Accept the stream on the authorized connection         |
  | Open TCP using only pinned loopback target addresses   |
  | Forward bytes between that TCP socket and the stream   |
  +--------------------------------------------------------+
     |
     v
  Local web application receives ordinary HTTP

RESPONSE — reverse direction of the same QUIC stream
  Local web application
    -> CLI copies TCP response bytes into the stream
    -> Saorsa encrypted transport
    -> phone gateway: Hyper parses the HTTP response
    -> gateway applies response policy (headers, cookies, redirects)
    -> browser
```

The phone gateway connects the browser's temporary session address to your local
app. It has three main responsibilities:

- **Check each request.** Only requests allowed for the current session are
  forwarded. The browser cannot change the local app selected by `--target`.
- **Adapt HTTP addresses.** The CLI securely tells the phone the app's hostname
  and port. The phone uses that information to adjust request headers and
  redirects—not to look up or connect to the app itself. The CLI makes that
  connection on the computer.
- **Pass through your app's content.** API writes, SSE and WebSockets are supported
  within resource limits. The app's own browser security policy (CSP) is preserved;
  no tunnel CSP is added. External resources load directly from the browser.
  URLs inside HTML, JavaScript and CSS are not rewritten.

This is access to one local app, not a general-purpose proxy or device-wide VPN.
For the detailed rules on headers, redirects and cookies, see
[HTTP adaptation](CLI.md#http-adaptation). For connection order, timeouts and why
requests are never replayed after connecting, see
[local HTTP target rules](CLI.md#local-http-target).

Admission, concurrent work, headers/bodies, per-stream byte forwarding and stalled
operations are bounded. Those limits are **not** a healthy-session timer or a
cumulative request-count ceiling. The first CLI TLS winner consumes the one-shot
attempt; a hostile first winner can deny availability even without passing the grant.

### What goes into a QUIC packet

Bodies can stream without buffering the entire response. Conceptually, the packet
layers are:

```text
UDP packet
  +-- Encrypted QUIC contents
       +-- Stream ID + position + some HTTP bytes
```

A request can span many packets, and a packet can carry frames from multiple
streams. QUIC handles packetization, retransmission and ordering; our gateway does
not implement those mechanisms. On Relay, the computer-side MASQUE carrier adds
an outer transport around these inner peer packets.

**This is HTTP/1.1 carried inside QUIC streams, not HTTP/3.** Opening a stream does
not require a new TLS handshake; requests share the authorized QUIC connection.

## 7. Who owns Stop

```text
Android Stop / service teardown       Terminal q / Ctrl-C
              |                               |
      Native cancellation owner          CLI owner
              |                               |
              +----- session work stops ------+
                              |
                Close streams and local sockets
                Retire owned transport endpoints
                Release unused/owned relay allocation
                Drain private runtime work
```

The phone installs a permanently closed unsolicited-ingress handler from endpoint
construction. The CLI uses bounded admission instead. Neither client serves as
a public relay, enables UPnP or advertises unwanted services.

Scanning has its own cancel/background/60-second expiry behavior; it does not limit
an established session. There is no automatic resurrection after Stop or failure.
A new connection requires a fresh CLI offer. Remote relay reservation retention
is upstream policy: withdrawing access does not promise immediate remote FD/quota
reclamation. Configured connection limits are not proof of a global resource cap.

## 8. Build machinery is not part of the live tunnel

```text
Repository + locked dependencies + reviewed patches
                         |
             prepare-native.sh
             Generated patched source workspace
             (Cargo's cached upstream stays untouched)
                         |
             native-workspace.sh
             Prepare fresh, or reject stale explicit reuse
                         |
               +---------+----------+
               |                    |
          Desktop binary       Android native library
                                    |
                             Source/artifact receipt
                                    |
                                Gradle APK
```

`scripts/native-workspace.sh` selects a prepared workspace for host builds/tests;
Android also uses the patched preparation and validates native staging receipts.
These scripts do not run in installed clients. Release automation packages binaries,
licenses and checksums and creates a draft for review; it does not supply routing,
discovery or a running relay. See [building](BUILDING.md) and [releasing](RELEASING.md).
