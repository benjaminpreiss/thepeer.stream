# thepeer.stream CLI

`crates/cli` (package `thepeer-cli`) builds the `thepeer` executable. Shared transport, grants,
HTTP policy and cleanup live in `crates/tunnel-core`; JNI lives in
`crates/tunnel-android`. No implementation is copied between packages.

## Build and run

Build from the repository root (see [prerequisites](BUILDING.md#prerequisites)):

```sh
bash scripts/build-helper.sh
```

Try a tiny localhost web app with an editable textbox. In a second terminal:

```sh
node -e '
  require("node:http").createServer((_req, res) => {
    res.setHeader("Content-Type", "text/html");
    res.end(`<input id="message" placeholder="Type here"><output id="echo"></output>
      <script>message.oninput=()=>echo.value=message.value</script>`);
  }).listen(3002, "127.0.0.1");
'
```

In a third interactive terminal, run `./target/build/debug/thepeer share localhost:3002`.
For the full test app instead, run `node tests/fixtures/application-app.mjs 3002`.
The build uses locked dependencies and reviewed transport patches. See
[build instructions](BUILDING.md#1-clone-and-fetch-dependencies) for initial fetching.

**Optional Relay:** Supply `--relay IP:PORT` to `share` only for a compatible
participant you have permission to use. Direct is tried first; there is no
default relay. See [validation history](../CHANGELOG.md) for tested routes.

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

Use matching CLI binaries on both computers, or a matching Android app. The
sharing terminal displays text by default; `r` shows its QR and `t` returns to
text. Both encode the **same one-shot offer**. The first peer to connect wins.

| Browser device | Pair | Open |
| --- | --- | --- |
| Computer | Run `thepeer connect`; paste the offer at the hidden prompt | Open the printed `http://thepeer-<nonce>.localhost:PORT` URL locally |
| Android | Tap **Connect** and scan with the in-app scanner | The app opens the browser |

The desktop browser gateway binds only to loopback. Keep the offer out of argv,
logs, files and screenshots; transfer text only via a trusted terminal. Redirected
CLI output is refused. The offer carries a grant and pin; see [wire policy](ROUTING-OFFER.md).

**Stop:** `q` or Ctrl-C on the sharing CLI, Ctrl-C on the connecting CLI, or
**Stop** in Android. Pair again with a fresh sharing offer after a disconnect;
there is no automatic reconnect.

**Server restarts:** You can pair before starting the web server. The healthy
tunnel survives server restarts and serves new requests when it returns;
in-flight HTTP requests or WebSockets may fail.

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

See [validation history](../CHANGELOG.md) for physical checks and remaining gates.
