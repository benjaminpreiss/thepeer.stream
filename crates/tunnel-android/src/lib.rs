//! Android JNI/foreground ownership only. Transport and HTTP policy live in tunnel-core.
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, Ordering};
use tunnel_core::{Failure, Result};
mod routing_owner;

// 0 idle, 1 reserved by foreground owner, 2 executing.
static STATE: AtomicU8 = AtomicU8::new(0);
static STOP: AtomicBool = AtomicBool::new(false);
static PORT: AtomicU16 = AtomicU16::new(0);

fn prepare() -> bool {
    if STATE
        .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return false;
    }
    STOP.store(false, Ordering::SeqCst);
    true
}
fn stop() {
    STOP.store(true, Ordering::SeqCst);
    routing_owner::stop();
    PORT.store(0, Ordering::SeqCst);
}
fn abandon_prepared() {
    stop();
    let _ = STATE.compare_exchange(1, 0, Ordering::SeqCst, Ordering::SeqCst);
}
fn port() -> u16 {
    if STOP.load(Ordering::SeqCst) {
        0
    } else {
        PORT.load(Ordering::SeqCst)
    }
}
struct Reservation;
impl Drop for Reservation {
    fn drop(&mut self) {
        PORT.store(0, Ordering::SeqCst);
        STATE.store(0, Ordering::SeqCst);
    }
}

// Existing Java method names remain ABI names, not scriptless-viewer entry points.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_webp2p_controllerprobe_NativeSaorsaProbe_nativeViewerPrepare(
    _: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
) -> jni::sys::jboolean {
    prepare().into()
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_webp2p_controllerprobe_NativeSaorsaProbe_nativeViewerStop(
    _: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
) {
    stop();
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_webp2p_controllerprobe_NativeSaorsaProbe_nativeViewerAbandonPrepared(
    _: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
) {
    abandon_prepared();
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_webp2p_controllerprobe_NativeSaorsaProbe_nativeViewerPort(
    _: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    port().into()
}
