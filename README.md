# thepeer.stream

> **Special thanks to [Andrew Thomas (@andrewmthomas87)](https://github.com/andrewmthomas87).**
> This project took inspiration from his [web-p2p-tunnel repository](https://github.com/andrewmthomas87/web-p2p-tunnel).

Post-quantum-secure, peer-to-peer tunneling for local services. Connect directly first, with optional relay fallback.

```text
         Computer                                      Phone
┌────────────────────────┐                  ┌────────────────────────┐
│ Local web application  │                  │        Browser         │
│           ↕            │                  │           ↕            │
│  ┌─ CLI ────────────┐  │                  │  ┌─ App ────────────┐  │
│  │    ▄▄▄ ▄ ▄▄▄     │  │                  │  │    ┌─      ─┐    │  │
│  │    █ █ ▀ █ █     │  │◄════ Tunnel ════►│  │                  │  │
│  │    ▀▀▀ ▄ ▀▀▀     │  │                  │  │    └─      ─┘    │  │
│  │  Scan to connect │  │                  │  │     Scan QR      │  │
│  └──────────────────┘  │                  │  └──────────────────┘  │
└────────────────────────┘                  └────────────────────────┘
                            Direct or Relay
```

## Roadmap

- [x] Open a computer’s local web application in an Android phone’s browser.
- [ ] mobile cross-platform support (iOS through flutter)
- [ ] Operate a community funded relay
- [ ] Terminal-to-terminal tunneling.
- [ ] Benchmark Direct and Relay performance; evaluate HTTP/3 where it offers measurable benefits.

## Installation

### CLI

Install the latest version with:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/benjaminpreiss/thepeer.stream/main/scripts/install.sh | sh
```

The [installer](scripts/install.sh) verifies the release checksum and installs
`thepeer` in `~/.local/bin`, without sudo. Add that directory to your `PATH`
if needed. Supports Linux x86_64 (glibc 2.35+) and macOS 15+ (Apple silicon/Intel).
It retains the archive containing licenses under `~/.local/share/thepeer.stream/`.

You can also install a draft release or build from source; see the
[installation guide](docs/INSTALLING.md).

### Phone app

Download the signed `android-arm64.apk` from
[GitHub Releases](https://github.com/benjaminpreiss/thepeer.stream/releases), open
it on your phone, and allow installation when Android prompts. Requires an arm64
phone running Android 8.0 or newer and a browser such as Chrome or Brave.

You can also install a draft release or build from source; see the
[installation guide](docs/INSTALLING.md).

## How to

### Share an app

On the computer running your web app (including over SSH):

```sh
thepeer share localhost:3000
```

Replace `3000` with the app’s port. Direct is tried first; for relay fallback,
add `--relay IP:PORT` using a compatible relay you have permission to use.
No relay is configured by default.

### Open it on your phone

Press `r` in the sharing terminal to show the QR. In the Android app, tap
**Connect** and scan it. The web app opens in your phone’s browser.

### Open it on another computer

Run `thepeer connect` on that computer. Paste the sharing terminal’s connection
string at the hidden prompt, then open the printed local URL in your browser.

Both CLIs stay open across dev-server restarts; new requests work when the server
returns. Stop either CLI with Ctrl-C (or tap **Stop** on Android). A broken tunnel
does not reconnect automatically.

## Features

- ✅ HTTP, API writes, cookies, local redirects, SSE and WebSockets within bounded HTTP/resource policies.
- ✅ No VPN or system proxy required.
- ✅ Local targets: `localhost:PORT`, `127.0.0.1:PORT`, `[::1]:PORT`, or a hostname that resolves only to loopback addresses.
- ✅ Hostnames resolved once before pairing.
- ❌ Remote targets, URL-form targets and HTTPS upstreams are not supported.

## Packages

```text
crates/tunnel-core/       Offers, planning, authentication, HTTP, owned cleanup
crates/tunnel-android/    JNI and foreground/Stop ownership
crates/cli/              Desktop CLI, local-address selection, terminal QR
apps/android-controller/ Android UI, scanner, service and Gradle
tests/fixtures/          Disposable localhost application for host/phone checks
```

- [Architecture: components, connection flow and Saorsa](docs/ARCHITECTURE.md)
- [Deploy a self-updating NixOS MASQUE relay](deployment/README.md)
- [Build from source](docs/BUILDING.md)
- [Build/run the CLI](docs/CLI.md)
- [Create a draft release](docs/RELEASING.md)
- [Build the Android app](apps/android-controller/README.md)
- [Validation status and known limitations](docs/STATUS.md)
- [Physical testing](docs/HUMAN-TESTING.md)
- [Offer and routing policy](docs/ROUTING-OFFER.md)
- [Release readiness](docs/RELEASE-READINESS.md)

**Keep live offers private.** Copy the connection string only through a trusted
terminal; never put it in a command, file, screenshot or log. Scan QRs with the
app, not a generic scanner that might send them to search.

## License

[MIT](LICENSE). See [third-party licensing](LICENSING.md) for dependency notices.
