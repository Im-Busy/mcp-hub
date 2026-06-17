//! AES-256-GCM credential encryption (Phase 2).
//!
//! Pattern from tool-cli: CredentialEnvelope with nonce+auth_tag+key_id.
//! Keys are auto-generated on first use and stored with 0o600 permissions.

/// Placeholder for encryption module.
pub fn encrypt(_plaintext: &str, _key: &[u8; 32]) -> Result<Vec<u8>, crate::error::HubError> {
    Err(crate::error::HubError::Encryption(
        "Encryption module not yet implemented".to_string()
    ))
}

pub fn decrypt(_ciphertext: &[u8], _key: &[u8; 32]) -> Result<String, crate::error::HubError> {
    Err(crate::error::HubError::Encryption(
        "Decryption module not yet implemented".to_string()
    ))
}
