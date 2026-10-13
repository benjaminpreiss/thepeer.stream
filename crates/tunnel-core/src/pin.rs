use rustls::{
    CertificateError, DigitallySignedStruct, Error, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use saorsa_transport::crypto::{
    pqc::types::MlDsaPublicKey,
    raw_public_keys::{
        RawPublicKeyVerifier, create_subject_public_key_info, fingerprint_public_key,
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Debug)]
pub(crate) struct PinnedVerifier {
    pub pin: [u8; 32],
    pub rejected: Arc<AtomicBool>,
}

// Only accept the canonical fixed-size SPKI emitted by the pinned Saorsa resolver.
// Avoid feeding arbitrary ASN.1 lengths to upstream's diagnostic RPK parser.
fn key(cert: &CertificateDer<'_>) -> Result<MlDsaPublicKey, Error> {
    let bytes = cert.as_ref();
    let start = bytes.len().checked_sub(1952).ok_or(bad_encoding())?;
    let key = MlDsaPublicKey::from_bytes(&bytes[start..]).map_err(|_| bad_encoding())?;
    let canonical = create_subject_public_key_info(&key).map_err(|_| bad_encoding())?;
    if canonical.as_slice() != bytes {
        return Err(bad_encoding());
    }
    Ok(key)
}
fn bad_encoding() -> Error {
    Error::InvalidCertificate(CertificateError::BadEncoding)
}

impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        cert: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let key = key(cert)?;
        if fingerprint_public_key(&key) != self.pin {
            self.rejected.store(true, Ordering::Relaxed);
            return Err(Error::InvalidCertificate(CertificateError::UnknownIssuer));
        }
        RawPublicKeyVerifier::new(vec![key]).verify_server_cert(
            cert,
            intermediates,
            name,
            ocsp,
            now,
        )
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        Err(Error::General("TLS 1.2 disabled".into()))
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        let key = key(cert)?;
        if fingerprint_public_key(&key) != self.pin {
            self.rejected.store(true, Ordering::Relaxed);
            return Err(Error::InvalidCertificate(CertificateError::UnknownIssuer));
        }
        // Real ML-DSA-65 signature verification; never assertion-only/skip-verification.
        RawPublicKeyVerifier::new(vec![key]).verify_tls13_signature(message, cert, signature)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ML_DSA_65]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_canonical_bounded_spki_is_parsed() {
        let public = MlDsaPublicKey::from_bytes(&[7u8; 1952]).unwrap();
        let canonical = create_subject_public_key_info(&public).unwrap();
        assert!(key(&CertificateDer::from(canonical.clone())).is_ok());
        for size in [0, 1, 1951, 1952] {
            assert!(key(&CertificateDer::from(vec![0u8; size])).is_err());
        }
        let mut extra = canonical.clone();
        extra.push(0);
        assert!(key(&CertificateDer::from(extra)).is_err());
        let mut bad_length = canonical;
        bad_length[1] = 0;
        assert!(key(&CertificateDer::from(bad_length)).is_err());
    }
}
