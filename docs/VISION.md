# thepeer.stream: browser access to a computer-localhost application

The MVP exposes one explicitly configured computer-localhost web application to a
normal browser on an Android phone or another computer. The installed Android app
or desktop `thepeer connect` CLI—not browser JavaScript—owns the Saorsa connection
and a fresh loopback browser origin. The sharing CLI owns the fixed target and
shows a one-shot text offer by default (`r` for the same offer as a QR, `t` for text).

The user explicitly pastes the offer at the desktop CLI's hidden prompt or scans
it with the Android app and can Stop access at any time. Authenticate the intended computer on the retained TLS connection before
sending the grant. Do not persist or log capabilities, reconnect automatically,
expose arbitrary destinations or impose a healthy-session timer.

Try eligible Direct candidates first, then a prepared single-carrier MASQUE Relay.
Coordinated punching can enable direct connectivity in additional NAT situations,
but is deliberately deferred for this release. Shared public IP is only a LAN hint,
not proof of reachability. No universal NAT or implicit dual-stack/NAT64 claim.

Use upstream transport functionality with the reviewed pinned patches rather than
build another traversal stack. Clients do not operate a relay, enable UPnP or
advertise unwanted services. Relay performance and participant discovery are later
work, not reasons to keep the retired browser-owned controller or public-relay
prototype in the product tree.

The desktop `thepeer connect` client uses the same one-shot authenticated offer and
bounded loopback-only HTTP gateway to browse a single explicitly configured localhost app. It does
not expose an unrestricted TCP proxy or shell. Other scoped protocols such as SSH
remain future possibilities.

See [current evidence](STATUS.md) and [release gates](RELEASE-READINESS.md).
