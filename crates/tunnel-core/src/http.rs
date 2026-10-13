//! Bounded application authorization framing; no diagnostic HTTP/viewer protocol.
#[cfg(feature = "host")]
#[path = "http_host.rs"]
pub mod host;
#[cfg(feature = "host")]
#[path = "http_inspector.rs"]
pub mod inspector;
use crate::{Failure, Result};
use saorsa_transport::high_level::{RecvStream, SendStream};
use std::time::Duration;
use tokio::time::timeout;
use zeroize::Zeroizing;
const MAX_RECORD: usize = 8192;

pub(crate) async fn write_frame(send: &mut SendStream, bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() || bytes.len() > MAX_RECORD {
        return Err(Failure("http-frame-length"));
    }
    send.write_all(&(bytes.len() as u32).to_be_bytes())
        .await
        .map_err(|_| Failure("http-write"))?;
    send.write_all(bytes)
        .await
        .map_err(|_| Failure("http-write"))
}
pub(crate) async fn read_frame(receive: &mut RecvStream) -> Result<Option<Zeroizing<Vec<u8>>>> {
    let mut prefix = [0u8; 4];
    match receive
        .read(&mut prefix[..1])
        .await
        .map_err(|_| Failure("http-read"))?
    {
        None => return Ok(None),
        Some(1) => {}
        _ => return Err(Failure("http-framing")),
    }
    let rest = async {
        receive
            .read_exact(&mut prefix[1..])
            .await
            .map_err(|_| Failure("http-truncated-prefix"))?;
        let length = u32::from_be_bytes(prefix) as usize;
        if length == 0 || length > MAX_RECORD {
            return Err(Failure("http-frame-length"));
        }
        let mut bytes = Zeroizing::new(vec![0; length]);
        receive
            .read_exact(&mut bytes)
            .await
            .map_err(|_| Failure("http-truncated-record"))?;
        Ok(Some(bytes))
    };
    timeout(Duration::from_secs(30), rest)
        .await
        .map_err(|_| Failure("http-frame-timeout"))?
}
