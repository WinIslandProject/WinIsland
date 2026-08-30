use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Errors that can occur during signing operations.
#[derive(Debug, thiserror::Error)]
pub enum SigningError {
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    #[error("Key error: {0}")]
    Key(String),
    #[error("Signature error: {0}")]
    Signature(String),
}

impl From<std::io::Error> for SigningError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<SigningError> for String {
    fn from(e: SigningError) -> Self {
        e.to_string()
    }
}

/// Compute SHA-256 digest of a file.
pub fn hash_file(path: &Path) -> Result<String, SigningError> {
    let data = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()))
}

/// Load an Ed25519 signing key from a PEM-encoded PKCS#8 file.
pub fn load_signing_key(path: &Path) -> Result<SigningKey, SigningError> {
    let pem = std::fs::read_to_string(path).map_err(|e| {
        SigningError::Key(format!("Cannot read key file '{}': {}", path.display(), e))
    })?;
    let key = SigningKey::from_pkcs8_pem(&pem).map_err(|e| {
        SigningError::Key(format!("Invalid PEM key in '{}': {}", path.display(), e))
    })?;
    Ok(key)
}

/// Load an Ed25519 signing key from a PEM-encoded environment variable.
pub fn load_signing_key_from_env(var: &str) -> Result<SigningKey, SigningError> {
    let pem = std::env::var(var)
        .map_err(|_| SigningError::Key(format!("Environment variable '{}' not set", var)))?;
    let key = SigningKey::from_pkcs8_pem(&pem)
        .map_err(|e| SigningError::Key(format!("Invalid PEM key in env '{}': {}", var, e)))?;
    Ok(key)
}

/// Sign a payload with the given signing key.
/// Returns the signature as a lowercase hex string.
pub fn sign_payload(key: &SigningKey, payload: &[u8]) -> String {
    let sig: Signature = key.sign(payload);
    hex::encode(sig.to_bytes())
}

/// Verify a signature against a payload and public key.
pub fn verify_signature(
    public_key: &VerifyingKey,
    payload: &[u8],
    signature_hex: &str,
) -> Result<(), SigningError> {
    let sig_bytes = hex::decode(signature_hex)
        .map_err(|_| SigningError::Signature("Invalid hex signature".into()))?;
    let sig = Signature::from_slice(&sig_bytes)
        .map_err(|_| SigningError::Signature("Invalid signature bytes".into()))?;
    public_key
        .verify_strict(payload, &sig)
        .map_err(|_| SigningError::Signature("Signature verification failed".into()))
}
