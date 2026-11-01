# thepeer.stream CLI

`crates/cli` (package `thepeer-cli`) builds the `thepeer` executable. Shared transport, grants,
HTTP policy and cleanup live in `crates/tunnel-core`; JNI lives in
`crates/tunnel-android`. No implementation is copied between packages.

## Build and run

With Rust, the pinned dependencies and native build prerequisites already cached:

```sh
bash scripts/build-helper.sh
node tests/fixtures/application-app.mjs 3002
# In another interactive terminal:
./target/build/debug/thepeer share localhost:3002
```

The build is locked/offline and applies the exact reviewed transport patch stack.
On a new machine, dependency fetching is a separate operator step:
`cargo +stable fetch --locked` from the repository root. No CLI or participant
is started by the build script. `CARGO_TARGET_DIR` can reuse an existing cache.

For optional Relay, supply `--relay IP:PORT` to `share` only with a compatible
service you are authorized to contact. No relay is configured by default. This
contacts the selected service; it does **not** operate a relay. Service address
and allocated relay contact are distinct. Allocation occurs before the offer is
shown. Relay on a terminal-to-terminal connection has not been physically verified.

## Local-address selection

`--bind` is optional. The CLI asks the OS for a source address toward the
explicit `--observer`, otherwise `--relay`. Without either it uses the numeric
IPv4 documentation address `192.0.2.1:9` as a routing-table hint. This temporary UDP
socket sends no discovery datagram; its port is discarded. The actual transport
binds the selected IP and obtains its own observation when configured.

VPNs and routing policy can select the wrong interface. Override explicitly:

```sh
./target/build/debug/thepeer share localhost:3002 --bind 192.168.2.120:0
```

No wildcard/loopback fallback or public-IP service is used. Mixed participant
families fail before discovery. Optional `--observer IP:PORT` supplies observation
only; coordinated punching is not supported.

## Local HTTP target

`share` accepts `hostname:PORT`, loopback IPv4 such as `127.0.0.1:PORT`, or
bracketed loopback IPv6 such as `[::1]:PORT`. The port must be nonzero. URLs,
credentials, paths, scoped IPv6 and HTTPS are unsupported. DNS names must be
ASCII, without a trailing dot.

- **Resolution:** The CLI resolves a hostname once through the computer's OS,
  before preparing participants or displaying the offer. Resolution has a five-second
  deadline and supports Stop. Empty answers, more than 16 addresses, or any
  non-loopback address reject the whole target.
- **Session addresses:** Validated addresses are saved for the session, in the
  order returned by the OS. There is no further resolution or IPv4/IPv6 preference.
  An explicit IP uses only that IP, without a hostname lookup.
- **Connections:** TCP establishment tries those addresses in order within a
  shared five-second deadline. Once connected, requests are never replayed against
  another address, even after an HTTP error or disconnect.

The target's address family is independent of the phone-to-computer transport's
address family. See the [startup resolution diagram](ARCHITECTURE.md#target-resolution-at-cli-startup).

## Pairing and ownership

Use the matching thepeer Android app or `thepeer connect` from the same release or source revision on the browsing computer. Run matching CLI binaries on both computers.
The sole offer is `wpa2:`. It retains the CLI's bound address, its ordinary
observation, and a separately allocated relay contact. See [wire policy](ROUTING-OFFER.md).
Old coordination-bearing offers (flag 1) are rejected, not translated.

The sharing terminal displays a fresh one-shot capability-bearing connection string
**by default**. Press `r` to display the Android QR and `t` to return to text;
both encode the same offer, and the first peer wins. Run `thepeer connect` on the
browsing computer in an interactive terminal and paste the text at its **hidden**
prompt; the CLI prints a `http://thepeer-…localhost:PORT` browser URL only after
authentication. The gateway binds only to loopback. Do not put the offer in argv,
logs, screenshots or files; copy it only through a trusted channel (such as your
SSH terminal). Redirected output is refused; scan the QR only with the Android
app's scanner, not a generic scanner. `q` or Ctrl-C on either CLI ends access;
Android has a **Stop** button. Start a fresh sharing CLI session for another peer.
The server may start after pairing or restart: a healthy tunnel and browser URL
remain available for new requests, but in-flight requests/WebSockets may fail.
A network/tunnel disconnect does not auto-reconnect; a new pairing needs a fresh
offer. No grant file exists.

The phone authenticates the intended computer on the **retained TLS connection**
before sending the grant. Observer connections never receive it. Only classified
no-response advances routing; TLS/authentication, input, observation, family,
Stop and cleanup errors are terminal.

Unsolicited handshakes are refused during preparation. Ready CLI admission is
bounded to four active and four queued attempts, with a ten-second active TLS
budget. This is not a proven global endpoint cap, sustained-load guarantee or
protection against a malicious first winner denying one-shot availability.

Healthy sessions have no lifetime or cumulative stream-count limit. Concurrent
requests, headers, bodies, directional forwarding, target operations and teardown
remain bounded. Stop/drop closes owned browser/transport resources. Relay withdrawal
is not a promise of immediate remote quota/FD reclamation (upstream ADR-011).

## HTTP adaptation

Your phone's browser uses a temporary session address, while your local app
expects its own address, such as `localhost:3000`. The gateway adjusts HTTP
headers and redirects to bridge that difference.

### Requests to your app

- **Check first, then translate.** The gateway checks that each request is allowed
  for this session, using its Host, Origin and browser fetch headers. Rewriting
  headers never turns a rejected request into an accepted one.
- **Use the app's address.** The Host header becomes the hostname and port you
  configured with `share`. An accepted Origin becomes `http://TARGET`. If no
  Origin was sent, none is added; a foreign or `null` Origin is rejected.
- **Discard claimed proxy information.** Browser-supplied `Forwarded`,
  `X-Forwarded-*` and `X-Real-IP` headers are removed.

WebSocket connections follow the same rules. Nothing in a browser request can
change which local application the CLI connects to.

### Redirects and cookies

- **Relative redirects**, such as `/login`, stay unchanged.
- **Full HTTP URLs** must name exactly your configured target or the current
  session's hostname and port. The gateway translates these redirects back to
  the browser's session address.
- **Other redirects are blocked:** HTTPS URLs, other destinations, URLs starting
  with `//`, and redirects containing backslashes or embedded whitespace/control
  characters. Checking a redirect never triggers a hostname lookup.
- **Cookies must belong to the current host.** Cookies without a `Domain`
  attribute are supported; those with one are rejected.

### Scripts, video and other resources

Your application's Content-Security-Policy (CSP), including report-only policies,
passes through unchanged. The gateway does not add its own CSP. External resources
load directly from the phone's browser, subject to your application's policies
and normal browser rules.

URLs inside HTML, JavaScript and CSS are not rewritten.

### Matching CLI and app versions

Use CLI and APK builds from the same release. After authorization, the CLI
sends a version-2 acknowledgement containing the configured target's hostname
and port. The phone rejects older or malformed acknowledgements.

## Host validation

```sh
bash scripts/test-native.sh
JAVA_HOME=/path/to/jdk bash apps/android-controller/test-native.sh
```

The second command requires the checksum-pinned local ZXing 3.5.3 jar; it downloads
nothing. It exercises the real CLI terminal QR → scanner → Java/JNI → browser HTTP/header checks
→ Stop path, plus scanner/handoff and receipt checks. All network fixtures are local.

For repeated checks, `THEPEER_PREPARED_WORKSPACE` may point to a trusted prepared tree;
tooling checks workspace sources, manifests, lockfiles and the preparation-input
receipt. Missing receipts or changed patch inputs require fresh preparation.
Do not modify Cargo's cached upstream checkout. Reuse a target directory and disable incremental builds
rather than accumulating duplicate build trees.

Earlier Android physical checks passed Direct on Wi-Fi and Relay on cellular,
including Stop and automatic binding. A later macOS-to-OrbStack Linux VM Direct
check exercised browsing before and after a dev-server restart, and the Android
QR path was checked. Terminal-to-terminal Relay has not been physically verified.
Relay slowness on the earlier cellular check is unmeasured; no universal NAT
reachability is claimed.
