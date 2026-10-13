# Route adapter: cancellation during active relay streams

Follow-up to [single-carrier routing](saorsa-route-relay.md). This is host-only,
loopback evidence. No upstream patch, public probe, Android activation or production
preparation changed.

**Later coverage:** [the negative-matrix follow-up](saorsa-route-negative-matrix.md)
adds four regressions (105 tests) without another implementation change. The counts
below record this active-stream slice.

## Failure observed before the fix

The new public-seam test prepares a helper with a real loopback relay, makes direct
unavailable, connects the phone, starts browser HTTP, and receives the first SSE event
from an independent localhost target. The target leaves the response unfinished.

Aborting the now-partially-polled `Connected::serve` task left the helper's target TCP
stream open beyond the five-second test observation window. The test failed twice,
including a filtered run taking about eleven seconds. The existing 98 tests passed;
unpolled-drop/socket-release tests had not exposed this active-stream failure.

The leading hypothesis was premature private-runtime destruction, rather than a new
transport requirement. The runtime owner previously exited its `block_on` as soon as
its stop sender dropped. Although the endpoint supervisor had also been signalled,
that supervisor raced runtime destruction. A connection close request did not ensure
that transport shutdown/draining would get execution time.

## Adapter-only correction

`route_runtime::run` now supplies a `TaskTracker` to its work closure. Endpoint shutdown
supervisors register with that tracker. When the runtime owner is dropped or explicitly
finished, the runtime stays alive while those supervisors finish, before cancelling
remaining runtime-owned tasks.

- Existing endpoint shutdown budget: eight seconds.
- New runtime drain budget: fifteen seconds, allowing helper allocation abort (up to
  five seconds) followed by endpoint shutdown.
- Existing final blocking-task shutdown budget: two seconds.
- Explicit runtime-owner completion budget: eighteen seconds (previously eight),
  covering drain, final shutdown and scheduling margin.

These budgets apply **after ownership ends**, never to a healthy session. The destructor
still does not spawn cleanup: it signals already-running supervisors. Explicit helper
and phone completion still propagate their endpoint/application failures. Forced runtime
teardown remains the bounded last resort; it cannot guarantee remote closure through
arbitrary packet loss.

The two active-stream tests passed after this change. Removing only the supervisor wait
in a generated diagnostic copy made both fail again. Restoring the wait made them pass.
This supports the teardown-ordering diagnosis; no packet capture or claim about every
possible transport shutdown cause is implied.

## Regression coverage at the approved seams

`native/src/peer_route_stream_tests.rs` uses `Helper::prepare_with_relay`, `connect`,
`Connected::serve`, Stop/drop, and observable real sockets:

1. **Drop phone serving during SSE.** A real event reaches the browser before abortion.
   The target closes before helper shutdown, the phone UDP socket becomes rebindable,
   the browser listener closes and the unfinished response ends. Separate helper and
   phone tokens remain uncancelled, so a shared Stop cannot make the assertion pass.
2. **Drop helper during WebSocket.** A real HTTP 101 upgrade and masked/unmasked binary
   frames cross the relay first. Dropping the helper closes the browser and target
   streams, terminates phone serving with `application-helper-closed`, releases the
   helper UDP socket and closes the browser gateway. Neither parent token is cancelled.
   The target accepts no replacement connection during the observation window.
3. **Stop during direct setup with both alternatives available.** Receipt of an actual
   Initial datagram establishes in-flight work without a timing sleep. Stop returns
   `peer-cancelled` after owned cleanup, releases the phone UDP socket and never accesses
   the target. The helper uses a distinct token and remains separately owned.

The third test initially imposed an unjustified two-second completion bound. This was
corrected to a ten-second *cleanup* observation bound: `connect` awaits owned teardown,
including upstream's bounded `wait_idle` drain. The test does not claim two-second UI
responsiveness or completion before the ordinary direct response deadline.

No assertion demands that the participant free its stable-port reservation; those
server-owned reservations retain the semantics documented in the preceding slice.

## Verification and limits

A fresh `bash experiments/direct-traversal-review/verify-peer-routing.sh` passed
**101 tests** (previous 98 plus these three). The three new tests also passed three
consecutive filtered runs. **96 legacy relay tests**, production-source host JNI
ABI/lifecycle checks, and four staging-validation tests passed again (no staging;
not Android). The generated counterfactual was removed.

Logs:
- `/tmp/wpt-route-stream-first.log`, `/tmp/wpt-route-stream-repeat.log`: SSE failure.
- `/tmp/wpt-route-stream-drain.log`, `/tmp/wpt-route-stream-two.log`: active-stream success.
- `/tmp/wpt-route-stream-counterfactual.log`: both active tests catch omitted draining.
- `/tmp/wpt-route-stream-three.log`: rejected two-second setup-cleanup assumption.
- `/tmp/wpt-route-stream-full.log`: fresh 101-test pass.
- `/tmp/wpt-route-stream-repeat-{1,2,3}.log`: repeated three-test passes.
- `/tmp/wpt-route-stream-{legacy,jni,staging}.log`: compatibility checks.

This adds real relayed SSE/WebSocket traffic and active drop coverage, not long-duration
cellular evidence or an exhaustive selector security matrix. Wrong-grant/TLS route
combinations, additional setup/overload/stream cancellation cases, hostile virtual-listener
bounds, public candidate acquisition, the versioned QR offer, and Android integration
remain pending. Existing lower-level authorization tests remain in the full suite.
