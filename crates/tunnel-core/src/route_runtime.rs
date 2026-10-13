//! Private runtime ownership for the review adapter. No transport patches.
//! The pinned P2pEndpoint shutdown can return with UDP still retained by tasks.
//! Graceful peer/endpoint cleanup happens first; ending this runtime releases
//! remaining runtime-owned tasks rather than leaking them into the caller's runtime.
use crate::{Failure, Result};
use std::{future::Future, time::Duration};
use tokio::{sync::oneshot, time::timeout};
use tokio_util::task::TaskTracker;

pub(super) struct Owner {
    stop: Option<oneshot::Sender<()>>,
    finished: oneshot::Receiver<()>,
}
impl Owner {
    pub(super) async fn finish(mut self) -> Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        timeout(Duration::from_secs(18), self.finished)
            .await
            .map_err(|_| Failure("route-runtime-timeout"))?
            .map_err(|_| Failure("route-runtime-failed"))
    }
}

// Dropping Owner drops its sender, so the worker exits without needing to spawn
// from a destructor. Its OS thread can finish even if the caller's runtime ends.
pub(super) async fn run<T, F, W>(work: W) -> Result<(T, Owner)>
where
    T: Send + 'static,
    F: Future<Output = Result<T>> + Send + 'static,
    W: FnOnce(TaskTracker) -> F + Send + 'static,
{
    let (ready, result) = oneshot::channel();
    let (stop, mut stopped) = oneshot::channel();
    let (done, finished) = oneshot::channel();
    let owner = Owner {
        stop: Some(stop),
        finished,
    };
    std::thread::Builder::new()
        .name("thepeer-owner".into())
        .spawn(move || {
            match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => {
                    runtime.block_on(async {
                        let cleanup = TaskTracker::new();
                        let output = tokio::select! {
                            biased;
                            _ = &mut stopped => None,
                            output = work(cleanup.clone()) => Some(output),
                        };
                        if let Some(output) = output {
                            if ready.send(output).is_ok() {
                                let _ = stopped.await;
                            }
                        }
                        // Drop requests cleanup, not immediate runtime destruction.
                        // Keep QUIC drivers alive while endpoint supervisors drain
                        // close frames. The helper may first abort its allocation.
                        cleanup.close();
                        let _ = timeout(Duration::from_secs(15), cleanup.wait()).await;
                    });
                    // Not a healthy-session limit: only reached after owner teardown.
                    // Bounded waiting for any blocking tasks; async tasks are cancelled.
                    runtime.shutdown_timeout(Duration::from_secs(2));
                }
                Err(_) => {
                    let _ = ready.send(Err(Failure("route-runtime")));
                }
            }
            let _ = done.send(());
        })
        .map_err(|_| Failure("route-runtime-thread"))?;
    match result.await {
        Ok(Ok(value)) => Ok((value, owner)),
        result => {
            let _ = owner.finish().await;
            Err(match result {
                Ok(Err(error)) => error,
                _ => Failure("route-runtime-failed"),
            })
        }
    }
}
