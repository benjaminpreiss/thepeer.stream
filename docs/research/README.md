# Research supporting the current implementation

Only investigations relevant to the Android-owned Direct/Relay tunnel are retained.
Irys/publication, browser-owned networking, hosted-controller delivery, alternate
transport products, relay hosting and coordinated-punching designs have been removed.

## Android and pairing

- [Native socket permissions and OS policy](android-native-socket-failure.md)
- [Why pairing uses the in-app scanner, not Brave's generic QR scanner](brave-android-qr-app-links.md)

## Transport selection and security

- [Upstream route selection and the selected Direct/Relay policy](saorsa-route-selection-reassessment.md)
- [Why the reviewed transport patches remain necessary](saorsa-upstream-divergence.md)
- [Inner peer TLS and relay trust boundaries](saorsa-upstream-relay-security.md)
- [Bounded public participant evidence](saorsa-public-participant-probe.md)
- [Retained-peer application authorization](saorsa-peer-application-integration.md)
- [Phase-aware dialing and conservative fallback](saorsa-phase-aware-peer-attempts.md)
- [Observed candidate provenance](saorsa-route-candidates.md)
- [Explicit address-family policy](saorsa-route-family-policy.md)
- [Single-carrier Relay ownership and remote reservations](saorsa-route-relay.md)

## Admission and lifecycle

- [Generic handshake ownership hooks](saorsa-handshake-hook-adapter.md)
- [Constructor-time helper admission](saorsa-preparation-admission.md)
- [Virtual ingress admission](saorsa-virtual-admission.md)
- [Phone ingress findings](saorsa-phone-ingress.md)
- [Bounded mixed-ingress contention](saorsa-paced-contention.md)
- [Authorization/Stop ordering](saorsa-authorization-stop.md)
- [Endpoint/runtime ownership](saorsa-route-adapter-lifecycle.md)
- [Active-stream teardown](saorsa-route-stream-teardown.md)
- [Failure/cancellation regression findings](saorsa-route-negative-matrix.md)

Some retained investigations contain dated paths, commands, test totals and
superseded intermediate designs. They are evidence for the current boundaries,
not runnable alternate packages or a backlog. In particular, the phone now uses
a permanently closed ingress policy, not a coordination phase gate. Follow
[current status](../STATUS.md), [build instructions](../BUILDING.md),
[offer policy](../ROUTING-OFFER.md) and [release gates](../RELEASE-READINESS.md)
for the implementation and current validation claims.
