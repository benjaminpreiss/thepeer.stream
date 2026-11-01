# CLI: share a localhost web app

Use matching `thepeer` binaries on both computers, or a matching Android app.
The CLI works in interactive terminals, including SSH sessions.

## Share from a computer

Run a local HTTP server. For a small page with an editable textbox, use Node.js:

```sh
node -e '
  require("node:http").createServer((_req, res) => {
    res.setHeader("Content-Type", "text/html");
    res.end(`<input id="message" placeholder="Type here"><output id="echo"></output>
      <script>message.oninput=()=>echo.value=message.value</script>`);
  }).listen(3002, "127.0.0.1");
'
```

In a second interactive terminal:

```sh
thepeer share localhost:3002
```

The CLI shows the one-shot connection string by default. Press `r` to show the
same offer as an Android QR; `t` returns to text. The server can also start
*after* sharing begins. For a richer test page, run
`node tests/fixtures/application-app.mjs 3002` instead of the Node example.

## Connect from another computer

Run `thepeer connect` on the browsing computer. Paste the sharing terminal's
connection string into its **hidden** prompt, then open the printed
`http://thepeer-<nonce>.localhost:PORT` URL in a browser **on that computer**.
The browser listener binds only to loopback.

## Connect from Android

Press `r` in the sharing terminal. In the matching Android app, tap **Connect**
and scan with the app's scanner. The app opens the web page in the phone browser.

## Stop and reconnect

The text and QR encode the **same** one-shot offer; the first peer to connect wins.
Use `q` or Ctrl-C on the sharing terminal, Ctrl-C on the connecting terminal,
or **Stop** in Android. A tunnel disconnect does not reconnect automatically:
start a fresh sharing session and pair with a new offer.

A healthy tunnel survives dev-server restarts. New requests work when the server
returns; in-flight HTTP requests and WebSockets may fail during the restart.

**Keep offers private:** transfer text via a trusted channel to the hidden prompt.
Never put a live offer in argv, files, logs or screenshots. Use the Android app's
scanner for the QR. Redirecting CLI output is refused.

## Build the CLI

From the repository root, after [fetching dependencies](BUILDING.md#1-clone-and-fetch-dependencies):

```sh
bash scripts/build-helper.sh
```

Run `./target/build/debug/thepeer` in place of `thepeer` above. For Android
builds and host test commands, see [BUILDING.md](BUILDING.md).

## Options and behavior

```text
Usage: thepeer share HOST:PORT [--bind IP:PORT] [--observer IP:PORT] [--relay IP:PORT]
       thepeer connect
```

- **Target:** `hostname:PORT`, `127.0.0.1:PORT` or `[::1]:PORT`. The hostname
  must resolve **only** to loopback addresses. The CLI resolves it once before
  pairing, pins those addresses for the session and forwards HTTP to that target.
  URLs and HTTPS targets are unsupported. For resolution and connection limits,
  see [target resolution](ARCHITECTURE.md#target-resolution-at-cli-startup).
- **Interface:** `share` selects a local source address automatically. If routing
  or a VPN selects the wrong one, use `--bind IP:PORT` (for example,
  `--bind 192.168.2.120:0`).
- **Optional services:** With `--relay IP:PORT`, a compatible participant you
  are authorized to use provides observation and Relay fallback. Without it,
  eligible Direct routes are tried alone. `--observer IP:PORT` configures
  observation without Relay. No public service is contacted by default.
  See [route policy](ROUTING-OFFER.md#route-selection).

### Local HTTP target

The target port must be nonzero. ASCII hostnames cannot have trailing dots;
resolution rejects empty results, more than 16 addresses, or any non-loopback
address. For each new TCP connection, the CLI tries saved addresses in OS order
within five seconds. An in-flight request is never replayed on another target
address after a failure.

### HTTP adaptation

The gateway checks the browser's Host and Origin, then translates accepted
requests to the configured target's hostname and port. Local redirects and
host-only cookies work; external redirects and cookie domains are rejected.
WebSockets follow the same request policy. Your app's CSP passes through;
URLs inside HTML, JavaScript and CSS are not rewritten. For the complete
header, redirect and cookie rules, see [browser request flow](ARCHITECTURE.md#6-how-a-browser-request-reaches-the-target).
