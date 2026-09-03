//! One signed container for everything Louis hands out: license keys and the
//! remote policy file.
//!
//! A signed blob is `SHELF1.<payload>.<signature>`, both halves base64url
//! without padding. The signature covers the raw payload bytes, so verifying
//! never has to re-serialize (and therefore never depends on key order or
//! whitespace surviving a round trip).

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::de::DeserializeOwned;

pub const PREFIX: &str = "SHELF1";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a Shelf signature")]
    Malformed,
    #[error("signature does not match")]
    BadSignature,
    #[error("public key is unusable: {0}")]
    BadPublicKey(String),
    #[error("contents could not be read: {0}")]
    BadPayload(String),
}

pub fn parse_public_key(hex: &str) -> Result<VerifyingKey, Error> {
    let bytes = decode_hex(hex.trim()).ok_or_else(|| Error::BadPublicKey("not hex".into()))?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| Error::BadPublicKey("expected 32 bytes".into()))?;
    VerifyingKey::from_bytes(&bytes).map_err(|e| Error::BadPublicKey(e.to_string()))
}

/// Checks the signature and hands back the decoded payload.
pub fn open<T: DeserializeOwned>(blob: &str, key: &VerifyingKey) -> Result<T, Error> {
    let mut parts = blob.trim().split('.');
    let (Some(PREFIX), Some(payload_b64), Some(sig_b64), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(Error::Malformed);
    };

    let payload = B64.decode(payload_b64).map_err(|_| Error::Malformed)?;
    let sig = B64.decode(sig_b64).map_err(|_| Error::Malformed)?;
    let sig: [u8; 64] = sig.try_into().map_err(|_| Error::Malformed)?;

    key.verify_strict(&payload, &Signature::from_bytes(&sig))
        .map_err(|_| Error::BadSignature)?;

    serde_json::from_slice(&payload).map_err(|e| Error::BadPayload(e.to_string()))
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

#[cfg(feature = "mint")]
mod mint {
    use super::{B64, PREFIX};
    use base64::Engine as _;
    use ed25519_dalek::{Signer, SigningKey};
    use serde::Serialize;

    pub fn seal<T: Serialize>(value: &T, key: &SigningKey) -> Result<String, serde_json::Error> {
        let payload = serde_json::to_vec(value)?;
        let sig = key.sign(&payload);
        Ok(format!(
            "{PREFIX}.{}.{}",
            B64.encode(&payload),
            B64.encode(sig.to_bytes())
        ))
    }
}

#[cfg(feature = "mint")]
pub use mint::seal;

pub fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
