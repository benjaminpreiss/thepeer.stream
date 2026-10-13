//! One-shot grant authorization used by the application session. No legacy GET
//! protocol, target fetching, grant-file writing, or alternate viewer entry point.
use anyhow::{Result, ensure};
use rand::RngCore;
use serde::Deserialize;
use zeroize::Zeroizing;

pub struct HttpInspector {
    expected: Option<blake3::Hash>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Authorization {
    v: u8,
    r#type: String,
    token: Zeroizing<String>,
}
impl HttpInspector {
    pub fn new(port: u16) -> Result<(Self, String)> {
        ensure!(port != 0, "choose a nonzero localhost port");
        let mut bytes = Zeroizing::new([0; 32]);
        rand::rngs::OsRng.try_fill_bytes(bytes.as_mut())?;
        let token = hex::encode(bytes.as_ref());
        Ok((
            Self {
                expected: Some(blake3::hash(token.as_bytes())),
            },
            token,
        ))
    }
    /// Every call consumes the one attempt, including malformed/wrong grants.
    pub async fn handle(&mut self, record: &[u8]) -> Result<()> {
        let expected = self.expected.take();
        ensure!(record.len() <= 8192, "authorization record too large");
        let request: Authorization = serde_json::from_slice(record)
            .map_err(|_| anyhow::anyhow!("invalid authorization request"))?;
        ensure!(
            request.v == 1
                && request.r#type == "authorize"
                && request.token.len() == 64
                && expected == Some(blake3::hash(request.token.as_bytes())),
            "authorization denied"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn only_one_valid_authorization_can_succeed() {
        let (mut gate, token) = HttpInspector::new(3002).unwrap();
        let record = Zeroizing::new(
            serde_json::to_vec(&serde_json::json!({"v":1,"type":"authorize","token":token}))
                .unwrap(),
        );
        assert!(gate.handle(&record).await.is_ok());
        assert!(gate.handle(&record).await.is_err());
    }
    #[tokio::test]
    async fn malformed_or_legacy_records_consume_the_attempt() {
        for bad in [
            b"{}".as_slice(),
            br#"{"v":1,"type":"get","id":1,"path":"/"}"#,
        ] {
            let (mut gate, token) = HttpInspector::new(3002).unwrap();
            assert!(gate.handle(bad).await.is_err());
            let record = Zeroizing::new(
                serde_json::to_vec(&serde_json::json!({"v":1,"type":"authorize","token":token}))
                    .unwrap(),
            );
            assert!(gate.handle(&record).await.is_err());
        }
    }
}
