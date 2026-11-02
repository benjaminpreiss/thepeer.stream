# Current status

The CLI shares one localhost web app with a matching desktop CLI or Android app.
The browsing device opens a local browser URL through a loopback gateway. The
sharing CLI owns the fixed target; the first peer consumes its one-shot offer.
See [features and roadmap](../README.md#features) and [pairing](CLI.md#connect-from-another-computer).

## Components

- `crates/tunnel-core`: offer, authentication, routing, HTTP and cleanup.
- `crates/cli`: `share` and `connect`, including the desktop browser gateway.
- `crates/tunnel-android` and `apps/android-controller`: JNI, scanner and Android session ownership.
- `crates/relay` and `deployment/`: optional Saorsa-compatible NixOS MASQUE relay and self-hosted deployment.
- `tests/fixtures` and `tests/host`: disposable application and process fixtures.

The route policy tries eligible Direct candidates first, then an explicitly
prepared single-carrier MASQUE Relay. Authentication uses the retained TLS
connection before sending the grant. Only a classified no-response permits
route fallback. Saorsa 0.37.0 is pinned at
`e2f08388088313c82cac393afa6ca27b9ce00516` with reviewed patches 0003–0005.
The [relay deployment](../deployment/README.md) has not been validated on a real host.

## Evidence and release gate

See [validation history](VALIDATION.md) for checks tied to specific builds and
unverified routes. Run the host suites in [BUILDING.md](BUILDING.md#tests) for
current results; snapshot test counts are not a release claim.

A version-tag workflow is designed to create draft assets. Validate its hosted
matrix, signing, licenses and actual assets before publication. Follow the
[release gate](RELEASE-READINESS.md) and [physical testing protocol](HUMAN-TESTING.md).
