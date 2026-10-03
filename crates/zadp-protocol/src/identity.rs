use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{
    Signature as Ed25519Signature, Signer, SigningKey, VerifyingKey, SIGNATURE_LENGTH,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

use crate::ProtocolError;

const SELLER_PREFIX: &str = "ed25519:";

/// Public seller identity (`ed25519:` + Base64 raw 32-byte key).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SellerId(String);

impl SellerId {
    pub fn from_verifying_key(key: &VerifyingKey) -> Self {
        Self(format!("{SELLER_PREFIX}{}", B64.encode(key.as_bytes())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn verifying_key(&self) -> Result<VerifyingKey, ProtocolError> {
        let raw = self
            .0
            .strip_prefix(SELLER_PREFIX)
            .ok_or_else(|| ProtocolError::InvalidSellerId("missing ed25519: prefix".into()))?;
        let bytes = B64
            .decode(raw)
            .map_err(|e| ProtocolError::InvalidSellerId(e.to_string()))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| ProtocolError::InvalidSellerId("public key must be 32 bytes".into()))?;
        VerifyingKey::from_bytes(&arr).map_err(|e| ProtocolError::InvalidSellerId(e.to_string()))
    }
}

impl std::fmt::Display for SellerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Base64-encoded ed25519 signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Signature(String);

impl Signature {
    pub fn from_bytes(bytes: &Ed25519Signature) -> Self {
        Self(B64.encode(bytes.to_bytes()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_ed25519(&self) -> Result<Ed25519Signature, ProtocolError> {
        let bytes = B64
            .decode(&self.0)
            .map_err(|e| ProtocolError::InvalidSignature(e.to_string()))?;
        if bytes.len() != SIGNATURE_LENGTH {
            return Err(ProtocolError::InvalidSignature(format!(
                "expected {SIGNATURE_LENGTH} bytes"
            )));
        }
        let mut arr = [0u8; SIGNATURE_LENGTH];
        arr.copy_from_slice(&bytes);
        Ok(Ed25519Signature::from_bytes(&arr))
    }
}

/// Seller signing keypair (identity key — not a Zcash key).
pub struct SellerKeyPair {
    signing: SigningKey,
    pub id: SellerId,
}

impl SellerKeyPair {
    pub fn generate() -> Self {
        let signing = SigningKey::generate(&mut OsRng);
        let id = SellerId::from_verifying_key(&signing.verifying_key());
        Self { signing, id }
    }

    /// Load a raw Ed25519 identity key from protected local storage. Not a Zcash key.
    pub fn from_secret_bytes(bytes: &[u8; 32]) -> Self {
        let signing = SigningKey::from_bytes(bytes);
        let id = SellerId::from_verifying_key(&signing.verifying_key());
        Self { signing, id }
    }

    pub fn secret_bytes(&self) -> [u8; 32] {
        self.signing.to_bytes()
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        let sig = self.signing.sign(message);
        Signature::from_bytes(&sig)
    }

    pub fn verify(
        id: &SellerId,
        message: &[u8],
        signature: &Signature,
    ) -> Result<(), ProtocolError> {
        let vk = id.verifying_key()?;
        let sig = signature.to_ed25519()?;
        vk.verify_strict(message, &sig)
            .map_err(|_| ProtocolError::SignatureMismatch)
    }
}
