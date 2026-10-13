# Phone ingress: constructor-installed refusal

## Relevant finding

The original host investigation found that a healthy Direct phone endpoint, using
default upstream construction, started unsolicited ordinary incoming TLS. Five
clients completed their side of genuine TLS while proxies withheld client Finished
from the phone. None was refused. The legitimate decoded-offer session still served
HTTP/body/CSP; unsolicited peers obtained no target access.

A constructor-installed refusal handler changed the probes to explicit connection
refusals while retaining legitimate outgoing HTTP and Stop behavior. This identified
an application admission gap, not an application authorization bypass or proof of
resource exhaustion. The general-purpose transport's default acceptance behavior
was not itself a violation of its contract.

## Current implementation

[PhoneIngress](../../crates/tunnel-core/src/phone_ingress.rs) is installed by
[route preparation](../../crates/tunnel-core/src/peer_route.rs) from endpoint
construction, before asynchronous bootstrap can admit unsolicited work. It:

- refuses ordinary incoming handshakes;
- refuses virtual incoming handshakes;
- discards coordinator-requested outgoing dials;
- leaves the client's explicit Direct/control/carrier connections intact.

The policy is permanently closed. There is no arm state, coordinated attempt owner
or handler-removal/reinstallation window in this MVP. The earlier phase-gate
implementation is removed. The existing reviewed constructor hook is sufficient;
no new transport patch or wire change is needed.

## Regression scope and limits

Current [session tests](../../crates/tunnel-core/tests/internal/peer_route_phone_ingress_tests.rs)
and [setup tests](../../crates/tunnel-core/tests/internal/peer_route_phone_setup_tests.rs)
exercise unsolicited ingress alongside legitimate connection/lifecycle operations.
They use real endpoints, refusal signals, browser forwarding and socket cleanup,
not synthetic successful TLS results. [Authorization/Stop ordering](saorsa-authorization-stop.md)
is a separate boundary: admission is not a substitute for pin/grant verification.

These bounded host checks do not establish sustained flood resistance, global
connection caps, fairness, CPU/RSS bounds or public NAT reachability. Constructor
ordering is source-enforced, not proof of every possible callback interleaving.
Upstream consumers that do not install a handler retain their normal behavior.
Private-runtime ownership still handles local endpoint retirement.

Patch 0005 remains unchanged; see [patch provenance](saorsa-handshake-hook-adapter.md).
Current physical evidence and remaining APK validation are tracked in
[release readiness](../RELEASE-READINESS.md), not inferred from these host probes.
