# Pairing offer and route policy

`thepeer share` creates one `wpa2:` offer for a localhost web app. The terminal
shows its text by default and can display the same offer as an Android QR. A
matching desktop CLI or Android app can connect; the first peer wins. Pair again
with a fresh offer after Stop or a tunnel disconnect.

## What the offer carries

- The sharing computer's public-key fingerprint, checked on the **actual retained
  TLS connection** before the browser peer sends the application grant.
- A fresh, one-shot application grant.
- Direct addresses and, if configured, a separate Relay contact and observation
  service address. Those are route hints, not proof of reachability.

**Treat text and QR as live credentials.** Transfer text over a trusted terminal
to `thepeer connect`'s hidden prompt; scan QR with the Android app. Never put an
offer in argv, files, logs or screenshots. Use matching binaries/apps from one
release or source revision.

## Route selection

1. If the offer names an observation service, obtain the browsing peer's
   address observation. Its failure ends the attempt.
2. Try eligible local Direct addresses when the peers share a public observation,
   or when no non-local candidate exists. Then try remaining Direct addresses,
   deduplicating identical sockets.
3. If Direct yields **no response**, try the allocated Relay contact when one
   was explicitly configured. Authentication or protocol failure ends the attempt
   instead of triggering fallback.

`--observer IP:PORT` supplies address observation only. `--relay IP:PORT`
prepares both observation and a single-carrier MASQUE Relay contact. With both
options, the explicit observer supplies observation and the configured relay
supplies forwarding. Local-only use needs no public participant. Shared public IP
is an imperfect LAN hint; it does not guarantee a route through all NATs.

## Wire format (for compatible clients)

ASCII `wpa2:` followed by canonical unpadded base64url. The decoded fields are:

| Offset | Bytes | Field |
| --- | ---: | --- |
| 0 | 1 | Version `2` |
| 1 | 1 | Flags: Relay `2`, bound address `4`, observer `8` |
| 2 | 32 | Sharing computer TLS key fingerprint |
| 34 | 32 | Application grant |
| 66 | variable | Primary Direct hint (observed address, otherwise bound address) |
| next, flag 2 | variable | Allocated Relay contact |
| next, flag 4 | variable | Original bound socket |
| next, flag 8 | variable | Observation service address |

An address is a family byte (`4` or `6`), four or sixteen IP octets and a
nonzero two-byte port. The offer holds at most four records: **73–142 decoded
bytes / 103–195 text bytes**, including the prefix. The decoder rejects unknown
unsupported flags, missing/trailing records, unusable addresses and noncanonical text before
network activity. The grant and encoded text are zeroized in memory.

The implementation lives in [`route_offer.rs`](../crates/tunnel-core/src/route_offer.rs)
and the planner in [`route_plan.rs`](../crates/tunnel-core/src/route_plan.rs).
See [validation](VALIDATION.md) for checked routes and [architecture](ARCHITECTURE.md)
for the full connection flow.
