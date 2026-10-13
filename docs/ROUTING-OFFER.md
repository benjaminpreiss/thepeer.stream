# Target routing offer

The helper and phone accept only **`wpa2:`**, not legacy `wpr1:`, `wpa1:` or
`wpt1:` formats. Android **0.6-direct-relay-preview** adds bounded-address and
observer hints. Older decoders reject the new flags: rebuild the native library
and APK before pairing with the rebuilt helper. No old format is translated.

Implementation: `crates/tunnel-core/src/route_offer.rs`, exposed through
`peer_route::Offer::{encode,decode}`. JNI calls `peer_route::connect`, the sole
Direct/Relay entry point. The historical three-route adapter has been removed.

## Interface and ownership

- `offer.encode()` returns a zeroizing, in-memory QR payload.
- `Offer::decode(&str)` performs bounded canonical validation without network,
  DNS, browser, filesystem or service activity.
- The phone acquires its own observation from the explicitly offered participant,
  if present, then plans eligible Direct candidates followed by Relay.
- Each attempted candidate must authorize the actual computer pin before sending
  the grant. Only classified no-response advances the plan. Authentication,
  protocol, observer/bootstrap, family, cancellation and cleanup errors are terminal.

The offer has no Debug, Clone or generic Serialize implementation. Its grant and
codec packet/text buffers are zeroizing. The caller must clear borrowed input.
Encoding does not renew a grant or extend the helper's lifetime. Healthy sessions
have no arbitrary lifetime; scanner expiry is separate.

**Encoded text and decoded bytes contain a live capability.** Never put them in
logs, argv, clipboard, screenshots, files, preferences, saved state or Intents.
Only live terminal display and explicit in-memory/anonymous-pipe transfer are intended.
Malformed offers return only the nonsecret `route-offer-format` stage.

## Wire layout

ASCII `wpa2:` followed by canonical unpadded base64url:

| Offset/order | Bytes | Meaning |
| --- | ---: | --- |
| 0 | 1 | Version `2` |
| 1 | 1 | Flags: relay `2`, bound address `4`, observer `8`; bit `1` is retired |
| 2 | 32 | Computer TLS public-key fingerprint |
| 34 | 32 | Application grant |
| 66 | variable | Primary Direct hint: observed address when a participant is used, otherwise bind |
| next, if flag 2 | variable | Computer's allocated MASQUE relay contact |
| next, if flag 4 | variable | Computer's original bound socket, retained alongside the observation |
| next, if flag 8 | variable | Ordinary participant service address used for observations |

Each address is family byte `4`/`6`, four/sixteen IP octets, then a nonzero
big-endian two-byte port. No DNS or interface scope/flow metadata is encoded.
At most four records: **73–142 decoded bytes; 103–195 text bytes**, including prefix.
Java, JNI and native decoder bounds agree. Unknown flags, missing records and trailing
bytes fail closed. Coordination-bearing offers (bit `1`) are rejected. Other
supported flag subsets remain ordinary `wpa2:` layouts, not a legacy decoder branch.

All hints, even unused ones, reject unspecified/multicast/broadcast addresses,
zero ports, IPv4-mapped IPv6, IPv6 link-local addresses, and nonzero scope/flow metadata.
Private and loopback addresses remain representable. Structure is not proof of
reachability or authority. Mixed families are representable; a selected operation
must match the phone's explicitly selected family. No NAT64 synthesis or automatic
family switch is added.

## Address roles and planning

The helper retains **both** its original bound socket and the participant's
observation of that same ordinary UDP endpoint. An explicit `--observer` supplies
the observation preferentially; otherwise the configured relay service does.
The separately allocated relay contact is never substituted for an observation.

With an observer hint, the phone opens a client-only ordinary connection and reads
that retained connection's observed address. It fully retires this observation
endpoint/runtime before application attempts. Only the observed **IP** is used as
context; its UDP port is not advertised or used as the phone's later dial socket.
Different NAT mappings/public-IP pools can therefore make the hint inaccurate.

The planner adapts Saorsa-core's conservative shared-WAN rule:

1. Try local-scope Direct candidates when the two ordinary observations share a
   non-local IP, or when no non-local Direct/Relay alternative exists.
2. Try the other Direct candidates, deduplicating identical Direct socket addresses.
3. Try the prepared relay contact, preserving its separate route provenance.

Shared public IP is only a hint: CGNAT, segmentation and changing mappings can cause
false positives or negatives. No matching-subnet guess is added. With no participant,
local-only use needs no public bootstrap. **With an explicitly offered observer,
its failure is terminal even if a LAN route might otherwise work.** Automatic
participant discovery and observer failover remain separate future work.

No grant is sent to the observer. Every application candidate gets its own owned
endpoint/runtime, retired before advancing. Coordinated punching is not a step in
this production plan. No route-latency race or live relay-to-Direct upgrade is added.

## Evidence

After the package/legacy cleanup, **89 core tests and four helper tests pass**.
They include nine planner checks, layout/truncation and retired-flag rejection,
actual decoded-offer Direct HTTP, observer→Relay HTTP/Stop, observer Stop/drop,
admission/authorization races, forged-signature positive control and resource release.
The old 157-test figure included retired designs and is not the current suite count.

The real helper terminal QR→scanner→Java/JNI→Direct HTTP/body/CSP→Stop suite also
passed. Validation logs are local artifacts, not release inputs.

The user separately confirmed Wi-Fi Direct, cellular Relay and Stop with the
pre-split 0.6 build and automatic binding. That is not post-refactor APK validation
or a performance/NAT guarantee. Repeat physical checks after rebuilding; see
[human testing](HUMAN-TESTING.md).
