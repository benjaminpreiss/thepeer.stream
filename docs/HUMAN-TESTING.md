# Physical test handoff

## What has already worked

The user rebuilt/restaged **0.6-direct-relay-preview / code 6** and confirmed:

- Same Wi-Fi: Direct works with automatic helper binding.
- Phone on cellular: single-carrier Relay works.
- Stop closes browser access on both routes.
- Cellular Relay is noticeably slow; this is qualitative, not a benchmark.

Those confirmations precede the Rust workspace/legacy cleanup. They must not be
relabelled as post-refactor device evidence. No Android SDK/NDK build or device
operation has been performed by the agent during this cleanup.

## Current host checkpoint

The shared core and separate desktop helper have 89 + 4 passing Rust tests.
The real helper terminal QR → scanner → Java/JNI → browser HTTP/body/CSP → Stop
suite passes, including automatic bind and socket release. These are local host
fixtures, not a physical camera, cellular benchmark or public NAT guarantee.

Current code has one Direct/Relay router, no coordination stage, no legacy offer
translation and a 195-byte `wpa2:` text bound. The foreground owner moved to
`crates/tunnel-android`; shared policy is in `crates/tunnel-core`. Transport remains
0.37.0 at the exact reviewed revision with unchanged patches 0001–0005.

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

3. Build the [desktop helper](HOST-HELPER.md), start the disposable application on
   port 3002, and start a fresh helper/QR. Try same-Wi-Fi Direct first.

4. For Relay, agree on the participant **before traffic**. The previously approved
   service is `207.148.94.42:10000`; host test success does not approve other public
   participants. Start a fresh helper with `--relay`, then test the phone on cellular.
   No public relay is deployed or operated by these instructions.

5. On each route check HTTP/scripts/API writes, cookies/local redirects, SSE and
   WebSockets. Confirm the displayed route, browser reopen, background/foreground
   behavior, and Stop through both app and notification. Stop must close access.
   Reconnect only explicitly with a fresh helper and fresh QR.

Never put live QRs or grants into screenshots, clipboard, logs, files, preferences,
saved state or Intents. Use the in-app scanner, not Brave's generic QR scanner.
Record only route, nonsecret failure stage and observed behavior.

Observer failure is terminal even if a LAN path might work. Shared public IP is only
a hint; no subnet guessing, implicit dual-stack/NAT64, discovery or universal NAT
success is promised. Do not weaken authentication/fallback to make a trial pass.
Performance diagnosis and new routing features are deferred until after the MVP.
