# thepeer.stream: browser access to a computer-localhost application

The MVP exposes one explicitly configured computer-localhost web application to a
normal phone browser. An installed Android app—not browser JavaScript—owns the
Saorsa connection and a fresh local browser origin. The CLI owns the
fixed target and displays a one-shot pairing QR.

The user explicitly connects, scans a trusted helper and can Stop access at any
time. Authenticate the intended computer on the retained TLS connection before
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

The shared Rust core permits future desktop clients or additional explicitly scoped
protocols such as SSH without duplicating transport/security policy. Those are
future possibilities, not implemented MVP interfaces or a reason to expose an
unrestricted TCP proxy now.

See [current evidence](STATUS.md) and [release gates](RELEASE-READINESS.md).
