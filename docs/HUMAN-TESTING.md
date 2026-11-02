# Physical testing protocol

Use this checklist for each candidate release. Record the tag or commit, CLI/APK versions, route and nonsecret outcomes in the [validation history](VALIDATION.md). Do not treat host fixtures or earlier builds as device evidence.

1. Build [matching CLIs](BUILDING.md) on both computers and restage/install the Android app using [Android build instructions](../apps/android-controller/README.md). Do not bypass native receipt validation.
2. With a localhost server (see [CLI example](CLI.md#share-from-a-computer)), run `thepeer share localhost:3002` on the host and `thepeer connect` on the browsing computer. Paste the offer at the hidden prompt. Browse the printed loopback URL. Try Direct on the same network first.
3. Test pairing before starting the server and browse again after restarting it. New requests should work; in-flight requests/WebSockets may fail. Stop either CLI and confirm access ends. A new connection needs a fresh sharing offer.
4. On Android, use the app's **Connect** scanner for the QR from a fresh sharing session. Check camera permission, navigation, API writes, cookies, SSE/WebSockets, background/foreground and **Stop** (app and notification).
5. If an operator authorizes a compatible participant, repeat Android and desktop tests with `share ... --relay IP:PORT`, including cellular where available. Record Direct and Relay separately; host fixtures are not physical Relay evidence.

Keep live offers out of argv, logs, files and screenshots. Use only a trusted terminal to transfer the text to the hidden prompt and the in-app scanner for QR. No test establishes universal NAT reachability.
