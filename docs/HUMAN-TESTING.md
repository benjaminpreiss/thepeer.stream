# Physical test handoff

## What has already worked

An earlier Android preview build (not current release evidence) was rebuilt/restaged and confirmed:

- Same Wi-Fi: Direct works with automatic helper binding.
- Phone on cellular: single-carrier Relay works.
- Stop closes browser access on both routes.
- Cellular Relay is noticeably slow; this is qualitative, not a benchmark.

Those Android confirmations preceded the Rust workspace/legacy cleanup and are
not proof of a current signed APK. Separately, macOS-to-OrbStack Linux VM Direct
browsing worked before and after a dev-server restart, and the Android QR path
was checked. Terminal-to-terminal Relay remains physically unverified.

## Current host checkpoint

Core/CLI and host PTY/HTTP restart fixtures, plus the helper terminal QR → scanner →
Java/JNI → browser HTTP/header checks → Stop suite have been exercised. Rerun tests
for current counts; old snapshot counts are not a current total. These are local host
fixtures, not a physical camera, cellular benchmark or public NAT guarantee.

Current code has one Direct/Relay router, no coordination stage, no legacy offer
translation and a 195-byte `wpa2:` text bound. The foreground owner moved to
`crates/tunnel-android`; shared policy is in `crates/tunnel-core`. Transport remains
0.37.0 at the exact reviewed revision with reviewed patches 0003–0005.

## Repeat after cleanup

1. Stop any old session. Outside the agent sandbox, from the repository root:

   ```sh
   bash apps/android-controller/native-build.sh
   ```

   Use the installed NDK or `ANDROID_NDK_HOME`. The locked/offline build stages
   the checked arm64 library and a fresh source/artifact receipt. Do not bypass
   stale receipt errors. Report build failures without secret material.

2. Open `apps/android-controller` in Android Studio, Sync, and perform a full
   **Run app** on the arm64 phone. Do not use Apply Changes. Check camera permission
   requires another Connect, and cancelling/backgrounding scanning does not connect.

3. Build the [CLI](CLI.md) on both computers (same revision), start the disposable
   application on port 3002, and run `thepeer share localhost:3002`. Text is shown
   by default; `r` shows the same offer as an Android QR and `t` returns to text.
   On the second computer run `thepeer connect` and paste at its hidden prompt,
   then browse the printed loopback URL. Try same-Wi-Fi Direct first. Test
   starting the server after pairing and restarting it; in-flight requests or
   WebSockets may fail, but new requests should work when it returns.

4. For Relay, agree on the participant **before traffic**. The previously approved
   service must be confirmed by the operator; host test success does not approve
   other public participants. Start a fresh `share` with `--relay IP:PORT`, then
   test Android on cellular and separately test terminal-to-terminal Relay.
   No public relay is deployed or operated by these instructions.

5. On each route check HTTP/scripts/API writes, cookies/local redirects, SSE and
   WebSockets. Confirm the displayed route, browser reopen, background/foreground
   behavior, and Stop through both app and notification. Stop must close access.
   A network/tunnel disconnect does not auto-reconnect. Pair again explicitly with
   a fresh sharing session and new offer; the first peer wins each offer.

Never put live offers in command arguments, screenshots, logs, files, preferences,
saved state or Intents. Transfer text only over a trusted channel to the hidden prompt. Use the in-app scanner, not Brave's generic QR scanner.
Record only route, nonsecret failure stage and observed behavior.

Observer failure is terminal even if a LAN path might work. Shared public IP is only
a hint; no subnet guessing, implicit dual-stack/NAT64, discovery or universal NAT
success is promised. Do not weaken authentication/fallback to make a trial pass.
Performance diagnosis and new routing features are deferred until after the MVP.
