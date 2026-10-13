//! One authenticated application; HTTP policy at the phone, opaque streams at the helper.
//! No destination is supplied by a browser or by a data stream. No HTTP request replay.
use crate::http::{read_frame, write_frame};
use crate::{Failure, Result};
#[cfg(all(test, feature = "host"))]
use crate::{WireMode, client_endpoint, connect};
use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited, combinators::BoxBody};
use hyper::{Request, Response, StatusCode, body::Incoming, header, service::service_fn};
use hyper_util::rt::{TokioIo, TokioTimer};
use saorsa_transport::{
    HighLevelConnection,
    high_level::{RecvStream, SendStream},
};
use std::{
    convert::Infallible,
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
#[cfg(all(test, feature = "host"))]
use tokio::net::TcpStream;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpListener,
    sync::Semaphore,
    time::timeout,
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use zeroize::Zeroizing;

const PARALLEL: usize = 16;
const RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const REQUEST_BYTES: usize = 8 * 1024 * 1024;
type BodyError = Box<dyn std::error::Error + Send + Sync>;
type Body = BoxBody<Bytes, BodyError>;

/// The only transport adapter: Saorsa's existing Tokio stream traits, no new crypto.
struct Duplex {
    send: SendStream,
    recv: RecvStream,
}
impl AsyncRead for Duplex {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.recv).poll_read(cx, buf)
    }
}
impl AsyncWrite for Duplex {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        AsyncWrite::poll_write(Pin::new(&mut self.send), cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        AsyncWrite::poll_flush(Pin::new(&mut self.send), cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        AsyncWrite::poll_shutdown(Pin::new(&mut self.send), cx)
    }
}

/// Applies also to upgraded connections, where HTTP body limits no longer apply.
struct BudgetIo<T> {
    inner: T,
    read: usize,
    written: usize,
}
impl<T> BudgetIo<T> {
    fn new(inner: T) -> Self {
        Self {
            inner,
            read: 0,
            written: 0,
        }
    }
}
impl<T: AsyncRead + Unpin> AsyncRead for BudgetIo<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let remaining = RESPONSE_BYTES.saturating_sub(self.read);
        if remaining == 0 {
            return Poll::Ready(Err(io::Error::other("stream budget")));
        }
        let limit = remaining.min(buf.remaining());
        let mut part = ReadBuf::new(buf.initialize_unfilled_to(limit));
        let result = Pin::new(&mut self.inner).poll_read(cx, &mut part);
        let n = part.filled().len();
        buf.advance(n);
        self.read += n;
        result
    }
}
impl<T: AsyncWrite + Unpin> AsyncWrite for BudgetIo<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let remaining = RESPONSE_BYTES.saturating_sub(self.written);
        if remaining == 0 {
            return Poll::Ready(Err(io::Error::other("stream budget")));
        }
        let result = Pin::new(&mut self.inner).poll_write(cx, &buf[..buf.len().min(remaining)]);
        if let Poll::Ready(Ok(n)) = result {
            self.written += n;
        }
        result
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

// A root guard cancels *all* spawned tasks even if its parent future is dropped.
struct Scope {
    tasks: TaskTracker,
    cancel: CancellationToken,
}
impl Scope {
    fn new() -> Self {
        Self {
            tasks: TaskTracker::new(),
            cancel: CancellationToken::new(),
        }
    }
    fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let cancel = self.cancel.clone();
        self.tasks.spawn(async move {
            tokio::select! { biased; _ = cancel.cancelled() => {}, _ = future => {} }
        });
    }
    async fn finish(&self) {
        self.cancel.cancel();
        self.tasks.close();
        let _ = timeout(Duration::from_secs(2), self.tasks.wait()).await;
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[cfg(all(test, feature = "host"))]
use saorsa_transport::Endpoint;
#[cfg(all(test, feature = "host"))]
use std::net::SocketAddr;
#[cfg(all(test, feature = "host"))]
struct EndpointGuard(Endpoint);
#[cfg(all(test, feature = "host"))]
impl Drop for EndpointGuard {
    fn drop(&mut self) {
        self.0.close(0u32.into(), b"application closed");
    }
}
// Own the exact checked/accepted connection. Closing it must not close a shared
// P2P endpoint (which may also own coordinator or relay control connections).
struct ConnectionGuard(HighLevelConnection);
impl std::ops::Deref for ConnectionGuard {
    type Target = HighLevelConnection;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.close(0u32.into(), b"application closed");
    }
}

pub struct Session {
    #[cfg(all(test, feature = "host"))]
    _endpoint: Option<EndpointGuard>,
    connection: ConnectionGuard,
    target: crate::target::Target,
    permits: Arc<Semaphore>,
}
impl Session {
    /// Authorize an already established upstream peer connection. The caller owns
    /// its P2P endpoint and must retain/shut it down separately. Check the actual
    /// TLS identity before opening ANY application stream or disclosing the grant.
    /// Failure/cancellation/drop closes this connection, never the shared endpoint.
    pub fn from_peer(
        connection: HighLevelConnection,
        pin: [u8; 32],
        token: &str,
    ) -> impl Future<Output = Result<Self>> + '_ {
        // Take ownership synchronously: dropping an unpolled setup also closes it.
        let connection = ConnectionGuard(connection);
        async move {
            use rustls::{
                client::danger::ServerCertVerifier,
                pki_types::{CertificateDer, ServerName, UnixTime},
            };
            let certs = connection
                .peer_identity()
                .ok_or(Failure("application-peer-identity"))?
                .downcast::<Vec<CertificateDer<'static>>>()
                .map_err(|_| Failure("application-peer-identity"))?;
            let verifier = crate::pin::PinnedVerifier {
                pin,
                rejected: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            };
            verifier
                .verify_server_cert(
                    certs.first().ok_or(Failure("application-peer-identity"))?,
                    &certs[1..],
                    &ServerName::try_from("peer")
                        .map_err(|_| Failure("application-peer-identity"))?,
                    &[],
                    UnixTime::now(),
                )
                .map_err(|_| Failure("pin-mismatch"))?;
            timeout(Duration::from_secs(10), Self::authorize(connection, token))
                .await
                .map_err(|_| Failure("application-auth-timeout"))?
        }
    }

    #[cfg(all(test, feature = "host"))]
    async fn open(
        local: SocketAddr,
        remote: SocketAddr,
        pin: [u8; 32],
        token: &str,
    ) -> Result<Self> {
        let (endpoint, rejected) = client_endpoint(local, pin, WireMode::Application)?;
        Self::open_on_endpoint(endpoint, rejected, remote, token).await
    }

    // Endpoint construction owns transport choice; authentication stays identical.
    #[cfg(all(test, feature = "host"))]
    async fn open_on_endpoint(
        endpoint: Endpoint,
        rejected: Arc<std::sync::atomic::AtomicBool>,
        remote: SocketAddr,
        token: &str,
    ) -> Result<Self> {
        let endpoint = EndpointGuard(endpoint);
        let connection = ConnectionGuard(connect(&endpoint.0, remote, &rejected).await?);
        let mut session = Self::authorize(connection, token).await?;
        session._endpoint = Some(endpoint);
        Ok(session)
    }

    async fn authorize(connection: ConnectionGuard, token: &str) -> Result<Self> {
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|_| Failure("application-auth-stream"))?;
        #[derive(serde::Serialize)]
        struct Authorization<'a> {
            v: u8,
            r#type: &'static str,
            token: &'a str,
        }
        let auth = Zeroizing::new(
            serde_json::to_vec(&Authorization {
                v: 1,
                r#type: "authorize",
                token,
            })
            .map_err(|_| Failure("application-auth"))?,
        );
        write_frame(&mut send, &auth).await?;
        send.finish()
            .map_err(|_| Failure("application-auth-finish"))?;
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Ack {
            v: u8,
            r#type: String,
            #[serde(rename = "targetAuthority")]
            target_authority: String,
        }
        let ack = read_frame(&mut recv)
            .await?
            .ok_or(Failure("application-auth-ack"))?;
        let ack: Ack = serde_json::from_slice(&ack).map_err(|_| Failure("application-auth-ack"))?;
        if ack.v != 2 || ack.r#type != "application-authorized" {
            return Err(Failure("application-auth-ack"));
        }
        // Authenticated HTTP metadata, never a destination the phone resolves.
        // The helper alone owns the pinned numeric sockets.
        let target = ack
            .target_authority
            .parse::<crate::target::Target>()
            .map_err(|_| Failure("application-auth-ack"))?;
        if target.authority() != ack.target_authority {
            return Err(Failure("application-auth-ack"));
        }
        Ok(Self {
            #[cfg(all(test, feature = "host"))]
            _endpoint: None,
            connection,
            target,
            permits: Arc::new(Semaphore::new(PARALLEL)),
        })
    }
}

fn error(status: StatusCode) -> Response<Body> {
    let mut response = Response::new(
        Full::new(Bytes::from_static(
            b"Application request unavailable or rejected.",
        ))
        .map_err(|e: Infallible| match e {})
        .boxed(),
    );
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

/// A fresh per-session hostname, not another cookie-sharing 127.0.0.1 port.
pub fn valid_hostname(host: &str) -> bool {
    host.strip_prefix("thepeer-")
        .and_then(|host| host.strip_suffix(".localhost"))
        .is_some_and(|nonce| {
            nonce.len() == 32
                && nonce
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}

fn request_allowed(req: &Request<Incoming>, authority: &str) -> bool {
    let h = req.headers();
    let origin = format!("http://{authority}");
    if req.uri().scheme().is_some()
        || req.uri().authority().is_some()
        || !req.uri().path().starts_with('/')
        || req.uri().path().starts_with("//")
        || req.uri().to_string().len() > 8192
        || req.uri().path().contains('\\')
        || h.get_all(header::HOST).iter().count() != 1
        || h.get(header::HOST).and_then(|v| v.to_str().ok()) != Some(authority)
        || h.get_all(header::ORIGIN).iter().count() > 1
    {
        return false;
    }
    if let Some(value) = h.get(header::ORIGIN) {
        if value != origin.as_str() {
            return false;
        }
    }
    if !matches!(
        req.method().as_str(),
        "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS"
    ) {
        return false;
    }
    if !matches!(req.method().as_str(), "GET" | "HEAD") && !h.contains_key(header::ORIGIN) {
        return false;
    }
    if let Some(site) = h.get("sec-fetch-site") {
        if site != "same-origin" && site != "none" {
            // Android browser launch can be a cross-site top-level root navigation.
            return req.method() == hyper::Method::GET
                && req.uri() == "/"
                && !h.contains_key(header::ORIGIN)
                && h.get("sec-fetch-mode").is_some_and(|v| v == "navigate")
                && h.get("sec-fetch-dest").is_some_and(|v| v == "document");
        }
    }
    true
}

fn strip_hop(headers: &mut header::HeaderMap, upgrade: bool) {
    let nominated: Vec<_> = headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(|n| header::HeaderName::from_bytes(n.trim().as_bytes()).ok())
        .collect();
    for name in nominated {
        if !upgrade || name != header::UPGRADE {
            headers.remove(name);
        }
    }
    for name in [
        "connection",
        "proxy-connection",
        "proxy-authorization",
        "proxy-authenticate",
        "keep-alive",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
    ] {
        headers.remove(name);
    }
    if upgrade {
        headers.insert(
            header::CONNECTION,
            header::HeaderValue::from_static("upgrade"),
        );
        headers.insert(
            header::UPGRADE,
            header::HeaderValue::from_static("websocket"),
        );
    }
}

fn response_policy(
    response: &mut Response<Incoming>,
    authority: &str,
    target: &crate::target::Target,
    upgrade: bool,
) -> std::result::Result<(), ()> {
    let h = response.headers_mut();
    if h.get_all(header::LOCATION).iter().count() > 1 {
        return Err(());
    }
    if let Some(location) = h.get(header::LOCATION) {
        let value = location.to_str().map_err(|_| ())?.trim();
        if value.contains('\\')
            || value.starts_with("//")
            || value
                .bytes()
                .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
        {
            return Err(());
        }
        if let Some(rest) = value.strip_prefix("http://") {
            let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
            let host = &rest[..end];
            if host != target.authority() && host != authority {
                return Err(());
            }
            let suffix = &rest[end..];
            let separator = if suffix.starts_with('/') { "" } else { "/" };
            let rewritten = format!("http://{authority}{separator}{suffix}");
            h.insert(header::LOCATION, rewritten.parse().map_err(|_| ())?);
        } else if value
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .contains(':')
        {
            // Reject other schemes, but allow colons inside relative paths/queries.
            return Err(());
        }
    }
    // Only host-only cookies in this MVP; never broaden them to .localhost.
    for cookie in h.get_all(header::SET_COOKIE) {
        if cookie
            .to_str()
            .map_err(|_| ())?
            .split(';')
            .skip(1)
            .any(|v| {
                v.trim()
                    .split('=')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .eq_ignore_ascii_case("domain")
            })
        {
            return Err(());
        }
    }
    strip_hop(h, upgrade);
    h.insert(
        header::REFERRER_POLICY,
        header::HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    h.insert(
        header::HeaderName::from_static("x-content-type-options"),
        header::HeaderValue::from_static("nosniff"),
    );
    h.insert(
        "permissions-policy",
        header::HeaderValue::from_static(
            "camera=(), microphone=(), geolocation=(), payment=(), usb=()",
        ),
    );
    // Browser resource policy belongs to the application. Preserve its CSP and
    // report-only headers without injecting an additional tunnel policy.
    Ok(())
}

async fn proxy(
    mut req: Request<Incoming>,
    session: Arc<Session>,
    authority: String,
    scope: Arc<Scope>,
) -> Response<Body> {
    if !request_allowed(&req, &authority) {
        return error(StatusCode::FORBIDDEN);
    }
    if req
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
        .is_some_and(|n| n > REQUEST_BYTES)
    {
        return error(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let upgrade = req
        .headers()
        .get(header::UPGRADE)
        .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"websocket"));
    if req.headers().contains_key(header::UPGRADE) && !upgrade {
        return error(StatusCode::BAD_REQUEST);
    }
    if upgrade
        && (req.method() != hyper::Method::GET
            || req
                .headers()
                .get(header::ORIGIN)
                .and_then(|v| v.to_str().ok())
                != Some(format!("http://{authority}").as_str()))
    {
        return error(StatusCode::FORBIDDEN);
    }
    let Ok(permit) = session.permits.clone().try_acquire_owned() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let browser_upgrade = hyper::upgrade::on(&mut req);
    let had_origin = req.headers().contains_key(header::ORIGIN);
    strip_hop(req.headers_mut(), upgrade);
    let forwarding: Vec<_> = req
        .headers()
        .keys()
        .filter(|name| {
            name.as_str() == "forwarded"
                || name.as_str().starts_with("x-forwarded-")
                || name.as_str() == "x-real-ip"
        })
        .cloned()
        .collect();
    for name in forwarding {
        req.headers_mut().remove(name);
    }
    let Ok(host) = header::HeaderValue::from_str(session.target.authority()) else {
        return error(StatusCode::BAD_GATEWAY);
    };
    req.headers_mut().insert(header::HOST, host);
    if had_origin {
        // Original Origin was validated above, before rewriting or stripping.
        let Ok(origin) = header::HeaderValue::from_str(&format!("http://{}", session.target))
        else {
            return error(StatusCode::BAD_GATEWAY);
        };
        req.headers_mut().insert(header::ORIGIN, origin);
    }
    let opened = timeout(Duration::from_secs(10), async {
        let (send, recv) = session.connection.open_bi().await.map_err(|_| ())?;
        let (mut sender, connection) = hyper::client::conn::http1::Builder::new()
            .max_buf_size(16384)
            .handshake(TokioIo::new(BudgetIo::new(Duplex { send, recv })))
            .await
            .map_err(|_| ())?;
        scope.spawn(async move {
            let _ = connection.with_upgrades().await;
        });
        let req = req.map(|body| Limited::new(body, REQUEST_BYTES));
        sender.send_request(req).await.map_err(|_| ())
    })
    .await;
    let Ok(Ok(mut response)) = opened else {
        return error(StatusCode::BAD_GATEWAY);
    };
    let switching = response.status() == StatusCode::SWITCHING_PROTOCOLS;
    if switching
        && (!upgrade
            || !response
                .headers()
                .get(header::UPGRADE)
                .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"websocket")))
    {
        return error(StatusCode::BAD_GATEWAY);
    }
    if response_policy(&mut response, &authority, &session.target, switching).is_err() {
        return error(StatusCode::BAD_GATEWAY);
    }
    if switching {
        let upstream_upgrade = hyper::upgrade::on(&mut response);
        scope.spawn(async move {
            let _permit = permit;
            if let Ok((browser, upstream)) = tokio::try_join!(browser_upgrade, upstream_upgrade) {
                let _ = tokio::io::copy_bidirectional(
                    &mut BudgetIo::new(TokioIo::new(browser)),
                    &mut BudgetIo::new(TokioIo::new(upstream)),
                )
                .await;
            }
        });
    } else {
        // The permit must outlive the response body, including SSE. Wrap the body.
        return response.map(|body| {
            HeldBody {
                body: Limited::new(body, RESPONSE_BYTES),
                _permit: permit,
            }
            .boxed()
        });
    }
    response.map(|body| Limited::new(body, RESPONSE_BYTES).boxed())
}

struct HeldBody {
    body: Limited<Incoming>,
    _permit: tokio::sync::OwnedSemaphorePermit,
}
impl hyper::body::Body for HeldBody {
    type Data = Bytes;
    type Error = BodyError;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<std::result::Result<hyper::body::Frame<Bytes>, BodyError>>> {
        Pin::new(&mut self.body).poll_frame(cx)
    }
    fn is_end_stream(&self) -> bool {
        hyper::body::Body::is_end_stream(&self.body)
    }
    fn size_hint(&self) -> hyper::body::SizeHint {
        hyper::body::Body::size_hint(&self.body)
    }
}

pub async fn drive(
    listener: TcpListener,
    session: Session,
    hostname: String,
    stop: impl Future<Output = ()>,
) -> Result<&'static str> {
    if !valid_hostname(&hostname) {
        return Err(Failure("application-origin"));
    }
    let authority = format!(
        "{hostname}:{}",
        listener
            .local_addr()
            .map_err(|_| Failure("application-listener"))?
            .port()
    );
    let session = Arc::new(session);
    let scope = Arc::new(Scope::new());
    let _cancel_on_drop = scope.cancel.clone().drop_guard();
    let sockets = Arc::new(Semaphore::new(32));
    let serve = async {
        loop {
            let (socket, source) = listener
                .accept()
                .await
                .map_err(|_| Failure("application-accept"))?;
            if !source.ip().is_loopback() {
                continue;
            }
            let Ok(permit) = sockets.clone().try_acquire_owned() else {
                continue;
            };
            let session = session.clone();
            let authority = authority.clone();
            let tasks = scope.clone();
            scope.spawn(async move {
                let _permit = permit;
                let service = service_fn(move |req| {
                    let session = session.clone();
                    let authority = authority.clone();
                    let tasks = tasks.clone();
                    async move { Ok::<_, Infallible>(proxy(req, session, authority, tasks).await) }
                });
                let _ = hyper::server::conn::http1::Builder::new()
                    .timer(TokioTimer::new())
                    .header_read_timeout(Duration::from_secs(5))
                    .max_headers(64)
                    .max_buf_size(16384)
                    .serve_connection(TokioIo::new(socket), service)
                    .with_upgrades()
                    .await;
            });
        }
    };
    let result = tokio::select! { biased;
        _ = stop => Ok("Stopped. Application access closed."),
        _ = session.connection.closed() => Err(Failure("application-helper-closed")),
        result = serve => result,
    };
    // Explicit cancellation breaks task references to Scope before waiting/dropping.
    scope.finish().await;
    session.connection.close(0u32.into(), b"application ended");
    result
}

#[cfg(all(test, feature = "host"))]
async fn serve_target(
    endpoint: &Endpoint,
    inspector: crate::http::inspector::HttpInspector,
    target_port: u16,
    counts: &mut crate::http::host::Counts,
) -> Result<()> {
    serve_target_scoped(endpoint, None, inspector, target_port, counts).await
}

#[cfg(all(test, feature = "host"))]
async fn serve_target_scoped(
    endpoint: &Endpoint,
    peer: Option<SocketAddr>,
    inspector: crate::http::inspector::HttpInspector,
    target_port: u16,
    counts: &mut crate::http::host::Counts,
) -> Result<()> {
    let result = serve_target_inner(
        endpoint,
        peer,
        inspector,
        crate::target::PreparedTarget::loopback(target_port),
        counts,
    )
    .await;
    if result.is_err() {
        counts.failed += 1;
        counts.publish();
    }
    result
}

#[cfg(all(test, feature = "host"))]
async fn serve_target_inner(
    endpoint: &Endpoint,
    peer: Option<SocketAddr>,
    inspector: crate::http::inspector::HttpInspector,
    target: crate::target::PreparedTarget,
    counts: &mut crate::http::host::Counts,
) -> Result<()> {
    use std::net::Ipv4Addr;
    let incoming = endpoint
        .accept()
        .await
        .ok_or(Failure("application-listener-closed"))?;
    let source_matches = match peer {
        Some(peer) => incoming.remote_address() == peer,
        None => incoming.remote_address().ip() == Ipv4Addr::LOCALHOST,
    };
    if !source_matches {
        incoming.refuse();
        return Err(Failure("application-source"));
    }
    counts.admitted += 1;
    counts.publish();
    let connection = timeout(Duration::from_secs(15), incoming)
        .await
        .map_err(|_| Failure("application-handshake-timeout"))?
        .map_err(|_| Failure("application-handshake"))?;
    serve_connection(ConnectionGuard(connection), inspector, target, counts).await
}

/// Serve one already accepted TLS connection, retaining its authoritative handle.
/// The endpoint owner must disable other application bidirectional-stream readers.
/// The fresh grant authorizes access before any fixed localhost target is opened.
#[cfg(feature = "host")]
pub fn serve_prepared_peer(
    connection: HighLevelConnection,
    inspector: crate::http::inspector::HttpInspector,
    target: crate::target::PreparedTarget,
    counts: &mut crate::http::host::Counts,
) -> impl Future<Output = Result<()>> + '_ {
    let connection = ConnectionGuard(connection);
    async move {
        counts.admitted += 1;
        counts.publish();
        let result = serve_connection(connection, inspector, target, counts).await;
        if result.is_err() {
            counts.failed += 1;
            counts.publish();
        }
        result
    }
}

#[cfg(all(test, feature = "host"))]
pub(crate) fn serve_peer(
    connection: HighLevelConnection,
    inspector: crate::http::inspector::HttpInspector,
    port: u16,
    counts: &mut crate::http::host::Counts,
) -> impl Future<Output = Result<()>> + '_ {
    serve_prepared_peer(
        connection,
        inspector,
        crate::target::PreparedTarget::loopback(port),
        counts,
    )
}

#[cfg(feature = "host")]
async fn serve_connection(
    connection: ConnectionGuard,
    mut inspector: crate::http::inspector::HttpInspector,
    target: crate::target::PreparedTarget,
    counts: &mut crate::http::host::Counts,
) -> Result<()> {
    let scope = Scope::new();
    let work = async {
        let (mut send, mut recv) = timeout(Duration::from_secs(30), connection.accept_bi())
            .await
            .map_err(|_| Failure("application-auth-timeout"))?
            .map_err(|_| Failure("application-auth-stream"))?;
        let auth = timeout(Duration::from_secs(10), read_frame(&mut recv))
            .await
            .map_err(|_| Failure("application-auth-timeout"))??
            .ok_or(Failure("application-auth-missing"))?;
        // Reuse the existing one-use, 256-bit grant policy, but not its GET protocol.
        inspector
            .handle(&auth)
            .await
            .map_err(|_| Failure("application-auth-denied"))?;
        let ack = serde_json::to_vec(
            &serde_json::json!({"v":2,"type":"application-authorized","targetAuthority":target.target.authority()}),
        )
        .map_err(|_| Failure("application-auth-ack"))?;
        write_frame(&mut send, &ack).await?;
        send.finish()
            .map_err(|_| Failure("application-auth-finish"))?;
        counts.authorized = true;
        counts.publish();
        let permits = Arc::new(Semaphore::new(32));
        loop {
            let accepted = tokio::select! {
                _ = connection.closed() => break,
                stream = connection.accept_bi() => stream,
            };
            let Ok((send, recv)) = accepted else {
                break;
            };
            let permit = permits
                .clone()
                .try_acquire_owned()
                .map_err(|_| Failure("application-concurrency"))?;
            counts.fetched = counts.fetched.saturating_add(1);
            counts.publish();
            let target = target.clone();
            scope.spawn(async move {
                let _permit = permit;
                // Only the pinned numeric loopback addresses; no DNS or replay.
                if let Ok(mut target) = target.connect().await {
                    let _ = tokio::io::copy_bidirectional(
                        &mut BudgetIo::new(Duplex { send, recv }),
                        &mut target,
                    )
                    .await;
                }
            });
        }
        Ok(())
    }
    .await;
    scope.finish().await;
    connection.close(0u32.into(), b"application ended");
    work
}

#[cfg(all(test, feature = "host"))]
#[path = "../tests/internal/application_peer_negative_tests.rs"]
mod peer_negative_tests;

#[cfg(all(test, feature = "host"))]
#[path = "../tests/internal/application_peer_tests.rs"]
mod peer_tests;

#[cfg(all(test, feature = "host"))]
#[path = "../tests/internal/application_tests.rs"]
mod tests;
