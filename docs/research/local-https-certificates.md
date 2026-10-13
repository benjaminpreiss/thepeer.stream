# HTTPS certificates for local applications

## Desktop development certificates

[mkcert](https://github.com/FiloSottile/mkcert) creates a local certificate authority,
installs its root into local trust stores and generates development certificates,
including for localhost. It does not configure the web server. This is local trust,
not Let's Encrypt issuance. Trust installed on a computer is not automatically
installed on a phone. The CA private key must not be distributed.

## Public certificates with a local destination

[Let's Encrypt's localhost guidance](https://letsencrypt.org/docs/certificates-for-localhost/)
says it cannot issue certificates for localhost. It distinguishes that name from
an owned public domain that resolves to 127.0.0.1: the latter can obtain a
certificate through DNS validation. It warns against distributing a certificate's
shared private key in a native application.

[DNS-01](https://letsencrypt.org/docs/challenge-types/#dns-01-challenge) proves control
using a public DNS TXT record under `_acme-challenge`, rather than contacting the
application's HTTP listener. A locally accessible service therefore need not be
publicly reachable to use a certificate for an owned public domain.

[Traefik's ACME configuration](https://doc.traefik.io/traefik/reference/install-configuration/tls/certificate-resolvers/acme/)
supports DNS-01 issuance and renewal through `acme.dnsChallenge`. It does not make
localhost eligible for a public certificate.

## Implication for thepeer

Serving HTTPS at the phone gateway is separate from HTTPS at the computer's local
application. The phone browser must trust the certificate for its session hostname.
A desktop development CA does not provide that trust automatically. Options include
explicit phone trust installation or a real-domain certificate issuance design;
neither is implemented by the current HTTP gateway. Never bundle a shared public
domain certificate private key or DNS API credentials in the APK.
