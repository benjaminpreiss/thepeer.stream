//! JNI foreground owner for the target-only router. No capability persistence.
use super::{PORT, Reservation, STATE, STOP};
use crate::{Failure, Result};
use std::{
    net::SocketAddr,
    sync::{
        Mutex,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tunnel_core::peer_route::{self, Offer, Route};
use zeroize::Zeroizing;

static ACTIVE: Mutex<Option<CancellationToken>> = Mutex::new(None);
static ROUTE: AtomicU8 = AtomicU8::new(0);

// Installing under the same lock used by Stop closes the pre-install race.
// Stop cancels synchronously, not through a separately scheduled Tokio watcher.
struct Active(CancellationToken);
impl Active {
    fn new() -> Self {
        let token = CancellationToken::new();
        let mut active = ACTIVE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *active = Some(token.clone());
        if STOP.load(Ordering::SeqCst) {
            token.cancel();
        }
        ROUTE.store(0, Ordering::SeqCst);
        Self(token)
    }
}
impl Drop for Active {
    fn drop(&mut self) {
        self.0.cancel();
        ACTIVE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        ROUTE.store(0, Ordering::SeqCst);
    }
}
pub(super) fn stop() {
    if let Some(token) = ACTIVE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
    {
        token.cancel();
    }
    ROUTE.store(0, Ordering::SeqCst);
}
pub(super) fn route() -> u8 {
    if super::port() == 0 {
        0
    } else {
        ROUTE.load(Ordering::SeqCst)
    }
}

pub(super) fn run(bytes: Zeroizing<Vec<u8>>, hostname: &str) -> String {
    if STATE
        .compare_exchange(1, 2, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return "Connection not reserved or already running.".to_owned();
    }
    let _reservation = Reservation;
    let active = Active::new();
    let attempt = || -> Result<()> {
        if active.0.is_cancelled() {
            return Ok(());
        }
        if !tunnel_core::application::valid_hostname(hostname) {
            return Err(Failure("application-origin"));
        }
        let offer =
            Offer::decode(std::str::from_utf8(&bytes).map_err(|_| Failure("route-offer-format"))?)?;
        drop(bytes);
        // The primary Direct hint selects one explicit family. No implicit
        // dual-stack switching or NAT64 synthesis; selected-family errors are terminal.
        let bind = if offer.direct_candidate().is_ipv4() {
            SocketAddr::from(([0, 0, 0, 0], 0))
        } else {
            SocketAddr::from(([0u16; 8], 0))
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Failure("runtime"))?;
        let result = runtime.block_on(async {
            // Bind before opening the session so a listener error cannot strand
            // an authenticated connection awaiting explicit teardown. Do not
            // publish or serve this listener until pin AND grant authentication.
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .map_err(|_| Failure("application-loopback-bind"))?;
            let port = listener
                .local_addr()
                .map_err(|_| Failure("application-loopback-address"))?
                .port();
            if port == 18787 {
                return Err(Failure("application-origin-collision"));
            }
            let connected = peer_route::connect(offer, bind, &active.0).await?;
            if !active.0.is_cancelled() && !STOP.load(Ordering::SeqCst) {
                ROUTE.store(
                    match connected.route() {
                        Route::Direct => 1,
                        Route::Relay => 3,
                    },
                    Ordering::SeqCst,
                );
                PORT.store(port, Ordering::SeqCst);
            }
            // Even a Stop immediately after authorization takes this path:
            // serve honors the setup token and awaits the owner's full cleanup.
            connected
                .serve(listener, hostname.to_owned(), &active.0)
                .await
        });
        PORT.store(0, Ordering::SeqCst);
        runtime.shutdown_timeout(Duration::from_secs(2));
        result
    };
    match attempt() {
        Ok(()) => "Connection stopped. Connect with a fresh helper QR.".to_owned(),
        Err(Failure(stage)) => format!("Connection ended: stage={stage}; details omitted."),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn web_p2p_routing_client_v2() -> bool {
    true
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_webp2p_controllerprobe_NativeSaorsaProbe_nativeRouteSupported(
    _: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
) -> jni::sys::jboolean {
    true.into()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_webp2p_controllerprobe_NativeSaorsaProbe_nativeRouteKind(
    _: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    route().into()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_webp2p_controllerprobe_NativeSaorsaProbe_nativeRouteRun<'caller>(
    mut env: jni::EnvUnowned<'caller>,
    _: jni::objects::JClass<'caller>,
    packet: jni::objects::JByteArray<'caller>,
    hostname: jni::objects::JString<'caller>,
) -> jni::objects::JString<'caller> {
    env.with_env(|env| -> jni::errors::Result<_> {
        let len = if packet.is_null() {
            0
        } else {
            packet.len(env).unwrap_or(0)
        };
        // Bound before copying; decode the canonical wpa2 text in exactly one place.
        // Java's finally also wipes the original array, including invalid lengths.
        let mut bytes = Zeroizing::new(if (103..=Offer::MAX_ENCODED_LEN).contains(&len) {
            env.convert_byte_array(&packet).unwrap_or_default()
        } else {
            Vec::new()
        });
        if (103..=Offer::MAX_ENCODED_LEN).contains(&len)
            && packet.set_region(env, 0, &vec![0i8; len]).is_err()
        {
            bytes = Zeroizing::new(Vec::new());
        }
        jni::objects::JString::from_str(env, run(bytes, &hostname.to_string()))
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}
