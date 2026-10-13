# Localhost targets in other tunnel clients

## Documented interfaces

- [ngrok's CLI](https://ngrok.com/docs/agent/cli#http) accepts a port or
  `address:port`. Its official examples include `ngrok http 8080`,
  `ngrok http servername.local:9000` and `ngrok http https://localhost:8443`.
  Its upstream target is not restricted to a numeric loopback address.
- [Cloudflare Quick Tunnels](https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/trycloudflare/)
  explicitly documents `cloudflared tunnel --url http://localhost:8080`.
  Users can supply a localhost URL instead of choosing a numeric address.

These documents establish support for hostname targets. They do not establish
ngrok's internal resolver, IPv4/IPv6 preference, or either product's exact retry
and connection-racing behavior. Do not infer those details from the command syntax.

## Implication for thepeer

Passing `localhost` through a normal hostname-capable networking API is a valid
approach; explicit numeric addresses are not required by HTTP or QUIC. Localhost
is normally resolved locally, rather than by a public DNS query.

Thepeer's narrower contract is an operator-selected local HTTP authority, not
arbitrary upstream URLs. It resolves a hostname once, rejects any non-loopback
answer and pins the numeric addresses for the session. Explicit loopback IPs
need no name resolution. This is a scope choice, not evidence that other clients'
hostname support is unsafe or slower. It concerns the helper's local TCP target,
independently of Direct/Relay peer routing. See the current
[target and HTTP policy](../CLI.md).

## HTTP hostname adaptation

[ngrok's HTTP documentation](https://ngrok.com/docs/universal-gateway/http#rewriting-the-host-header)
describes optional request `Host` rewriting using its Traffic Policy `add-headers`
action. Setting `Host` replaces the existing value rather than appending another.
When Host rewriting is enabled, ngrok also automatically rewrites response
`Location` headers to match the endpoint URL's hostname.

This is configurable HTTP behavior, separate from resolving or connecting to the
upstream. That documentation does not establish automatic Origin, cookie-domain
or embedded HTML/JavaScript URL translation; do not infer those from Host rewriting.
For thepeer, strict Origin validation and the choice to translate accepted origins
remain application security decisions, not behavior inherited from ngrok.

## Browser resource policy

[ngrok's Add Headers action](https://ngrok.com/docs/traffic-policy/actions/add-headers)
explicitly supports adding response headers through `on_http_response`, including
operator-chosen security controls. Existing headers are appended to, not replaced
(except Host); replacing a header requires removing it first. This allows an
operator to configure CSP rather than treating browser resource restrictions as
an inherent requirement of HTTP tunneling.

The reviewed [HTTP endpoint documentation](https://ngrok.com/docs/gateway/endpoints/http)
does not document a mandatory same-origin-only CSP on forwarded application pages.
That is a documentation finding, not a live verification of every ngrok plan,
interstitial or endpoint configuration. External video requests are made by the
browser directly to the media provider, subject to browser rules and the site's
policies; they do not need to pass through the application tunnel.

