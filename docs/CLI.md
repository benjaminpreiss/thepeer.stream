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
./target/build/debug/thepeer --target localhost:3002
```

The build is locked/offline and applies the exact reviewed transport patch stack.
On a new machine, dependency fetching is a separate operator step:
`cargo +stable fetch --locked` from the repository root. No CLI or participant
is started by the build script. `CARGO_TARGET_DIR` can reuse an existing cache.

To prepare Relay with the previously approved participant:

```sh
./target/build/debug/thepeer --target localhost:3002 --relay 207.148.94.42:10000
```

That command contacts the selected public service. It does **not** operate a relay.
Do not probe other participants without agreement. Service address and allocated
relay contact are distinct. Allocation occurs before the QR is published.

## Local-address selection

`--bind` is optional. The CLI asks the OS for a source address toward the
explicit `--observer`, otherwise `--relay`. Without either it uses the numeric
IPv4 documentation address `192.0.2.1:9` as a routing-table hint. This temporary UDP
socket sends no discovery datagram; its port is discarded. The actual transport
binds the selected IP and obtains its own observation when configured.

VPNs and routing policy can select the wrong interface. Override explicitly:

```sh
./target/build/debug/thepeer --bind 192.168.2.120:0 --target localhost:3002
```

No wildcard/loopback fallback or public-IP service is used. Mixed participant
families fail before discovery. Optional `--observer IP:PORT` supplies observation
only; coordinated punching is not supported.

## Local HTTP target

`--target` accepts `hostname:PORT`, loopback IPv4 such as `127.0.0.1:PORT`, or
bracketed loopback IPv6 such as `[::1]:PORT`. The port must be nonzero. URLs,
credentials, paths, scoped IPv6 and HTTPS are unsupported. DNS names must be
ASCII, without a trailing dot.

- **Resolution:** The CLI resolves a hostname once through the computer's OS,
  before preparing participants or publishing the QR. Resolution has a five-second
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

Use the matching thepeer Android app from the same release or source revision.
The sole offer is `wpa2:`. It retains the CLI's bound address, its ordinary
observation, and a separately allocated relay contact. See [wire policy](ROUTING-OFFER.md).
Old coordination-bearing offers (flag 1) are rejected, not translated.

The terminal displays a fresh capability-bearing QR, never a plaintext fallback.
Redirected output is refused. Scan only with the app's scanner, not Brave. Never
save, log, screenshot or copy the QR. `q` or Ctrl-C removes the surface and stops
the CLI. A selected TLS winner consumes the one-shot offer; start a fresh CLI session
for another Connect. No grant file or automatic reconnect exists.

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
  configured with `--target`. An accepted Origin becomes `http://TARGET`. If no
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
tooling rejects stale workspace sources/manifests/lockfiles. Do not modify Cargo's
cached upstream checkout. Reuse a target directory and disable incremental builds
rather than accumulating duplicate build trees.

Pre-split physical checks passed Direct on Wi-Fi and Relay on cellular, including
Stop and automatic binding. Relay slowness is known but unmeasured; optimization
and discovery changes are deferred until after the MVP.
