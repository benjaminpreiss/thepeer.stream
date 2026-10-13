# Paced mixed-ingress overload: fixed-budget recovery

One additional integration case extends the existing mixed-wave fixtures, not the
transport interface or application behavior. **145 routing tests pass.**

Source: [peer_route_contention_tests.rs](../../crates/tunnel-core/tests/internal/peer_route_contention_tests.rs),
using the existing real-wire `MixedWave` fixture in
[peer_route_admission_tests.rs](../../crates/tunnel-core/tests/internal/peer_route_admission_tests.rs).
Run current tests through `bash scripts/test-native.sh`. Counts and scenarios below
record the original investigation, including since-removed coordinated routing.

## Fixed traffic budget

Two windows on the same helper, reversing ordinary/virtual ordering in the second:

1. Four real unfinished handshakes occupy active capacity, four more wait, and one
   overflow attempt on each ingress receives `CONNECTION_REFUSED`.
2. Forty fresh attempts alternate ordinary and allocated virtual contacts. After each
   response the sender sleeps 125ms: no catch-up burst, at most eight new attempts per
   second. Every attempt must receive actual refusal within one second. The original
   four queued attempts must still have received no response.
3. The paced window must finish within eight seconds of starting the wave, before the
   ten-second active deadline. This is a fixture-validity check, not a throughput claim.
4. Stop generating fresh attempts, require the original queued clients to complete
   genuine client-side TLS on promotion, then allow their active handshakes to expire.
   Proxies withhold client Finished and retransmissions; the helper admits/authorizes none.

Total: **100 probe attempts**—16 active/queued occupants and 84 refused overflow attempts,
including 80 paced arrivals. Normal control connections and the subsequent legitimate
phone connection are additional. There are no public contacts or capability logs.

After both windows, one explicit phone connection uses the original decoded offer,
without a helper restart, replacement grant or automatic retry. Real browser HTTP returns
200, `recovered`, and CSP. Stop closes the browser listener and releases helper/phone UDP
binds; the independent synthetic relay remains running until fixture cleanup.

The entire case is bounded to 85 seconds. The focused passing run took about 50 seconds,
mostly waiting for the two active/queued expiry cycles.

## Sensitivity and receipts

A generated-only mutation discarded/refused queued work when an active handshake expired.
The test failed with:

```text
queued work must not be discarded with a late refusal: ConnectionClosed(... CONNECTION_REFUSED ...)
```

The mutation was restored before the fresh 145-test routing run. No production fix or
new transport patch was needed. Patch 0005 remains
`5f4f7e02a92fd01e5bcb721958b633574e1d8da8eeeacd6475803886cd19e386`.

Logs: `/tmp/wpt-paced-contention-{first,queue-mutation,full,helper,helper-retry}.log`.
The helper-suite rerun initially hit `No space left on device` during compilation, not
a test failure. The shared review build measured 151 GB, including 55 GB of incremental
cache. With explicit user approval, only `target/relay-policy-review/build/debug/incremental`
was removed, freeing about 56 GiB. Sources, prepared workspaces, binaries and receipts
were preserved. Retrying with `CARGO_INCREMENTAL=0` **passed the helper process suite**.
The coordinated-client checkpoint remains 91.

## What this does not establish

This checks admission bounds, preservation of queued work and recovery **after** paced
overload. New arrivals pause while the queue is promoted and expires. It is therefore
**not continuous sustained traffic across expiry boundaries**, fairness for a legitimate
client arriving while saturated, or a CPU/RSS/resource-exhaustion benchmark. Both kinds
of ingress are exercised from one synthetic client, not a many-identity attack.

Coordinated outgoing-dial contention and stronger continuous-load tests remain separate.
There is no claim of public NAT reachability, upstream readiness or a global twelve-
connection cap. Android migration, production patch promotion and physical testing
remain unchanged/deferred.
