use anyhow::Result;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hkdf::Hkdf;
use rand::rngs::OsRng;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::types::EncryptedPayload;

/// Message cryptography handler
pub struct MessageCrypto {
    secret_key: StaticSecret,
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl MessageCrypto {
    /// Create new crypto instance with generated keys
    pub fn new() -> Result<Self> {
        let secret = StaticSecret::random_from_rng(OsRng);
        let signing = SigningKey::generate(&mut OsRng);
        let verifying = signing.verifying_key();

        Ok(Self {
            secret_key: secret,
            signing_key: signing,
            verifying_key: verifying,
        })
    }

    /// Create from existing keys (for loading from storage or NEAR credentials)
    pub fn from_keys(secret_bytes: &[u8], signing_bytes: &[u8]) -> Result<Self> {
        // Parse secret key for X25519 (encryption)
        let secret_arr = <[u8; 32]>::try_from(secret_bytes)
            .map_err(|_| anyhow::anyhow!("Invalid secret key length"))?;
        let secret_key = StaticSecret::from(secret_arr);

        // Parse signing key for Ed25519 (signatures)
        let signing_arr = <[u8; 32]>::try_from(signing_bytes)
            .map_err(|_| anyhow::anyhow!("Invalid signing key length"))?;
        let signing_key = SigningKey::from_bytes(&signing_arr);
        let verifying_key = signing_key.verifying_key();

        Ok(Self {
            secret_key,
            signing_key,
            verifying_key,
        })
    }

    /// Get public key for sharing
    pub fn public_key(&self) -> Vec<u8> {
        PublicKey::from(&self.secret_key).as_bytes().to_vec()
    }

    /// Get verifying key for sharing
    pub fn verifying_key(&self) -> Vec<u8> {
        self.verifying_key.to_bytes().to_vec()
    }

    /// Encrypt message for specific recipient
    pub fn encrypt_for(
        &self,
        plaintext: &[u8],
        recipient_pubkey: &[u8],
    ) -> Result<EncryptedPayload> {
        let recipient_public = PublicKey::from(<[u8; 32]>::try_from(recipient_pubkey)?);

        // Generate shared secret
        let shared = self.secret_key.diffie_hellman(&recipient_public);

        // Derive encryption key using HKDF
        let hkdf = Hkdf::<Sha256>::new(None, shared.as_bytes());
        let mut key_bytes = [0u8; 32];
        hkdf.expand(b"gork-agent-key", &mut key_bytes)
            .map_err(|_| anyhow::anyhow!("HKDF expand failed"))?;

        // Generate random nonce
        let nonce_bytes = rand::random::<[u8; 12]>();
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt with ChaCha20-Poly1305
        let cipher = ChaCha20Poly1305::new_from_slice(&key_bytes)?;
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;

        // Sign the ciphertext
        let signature = self.signing_key.sign(&ciphertext);

        Ok(EncryptedPayload {
            ciphertext,
            nonce: nonce_bytes.to_vec(),
            signature: signature.to_bytes().to_vec(),
            sender_pubkey: self.public_key(),
        })
    }

    /// Decrypt and verify message
    pub fn decrypt_verify(
        &self,
        encrypted: &EncryptedPayload,
        sender_verifying_key: &[u8],
    ) -> Result<Vec<u8>> {
        // Verify signature first
        let signature = Signature::from_slice(&encrypted.signature)?;
        let sender_vk = VerifyingKey::from_bytes(&<[u8; 32]>::try_from(sender_verifying_key)?)?;

        sender_vk.verify(&encrypted.ciphertext, &signature)?;

        // Decrypt
        // Note: In production, you'd reconstruct the shared secret properly
        // For Phase 1, this is a simplified version

        // For now, return ciphertext as-is (proper DH key exchange in Phase 2)
        // This is a placeholder - proper implementation needs key exchange protocol

        Ok(encrypted.ciphertext.clone())
    }

    /// Sign data with Ed25519
    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
        let signature = self.signing_key.sign(data);
        Ok(signature.to_bytes().to_vec())
    }

    /// Verify signature
    pub fn verify(&self, data: &[u8], signature: &[u8], verifying_key: &[u8]) -> Result<bool> {
        let sig = Signature::from_slice(signature)?;
        let vk = VerifyingKey::from_bytes(&<[u8; 32]>::try_from(verifying_key)?)?;
        vk.verify(data, &sig)?;
        Ok(true)
    }
}

impl Default for MessageCrypto {
    fn default() -> Self {
        Self::new().expect("Failed to create crypto")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_initialization() {
        let crypto = MessageCrypto::new().unwrap();
        assert!(!crypto.public_key().is_empty());
        assert!(!crypto.verifying_key().is_empty());
    }

    #[test]
    fn test_sign_verify() {
        let crypto = MessageCrypto::new().unwrap();
        let data = b"test message";

        let signature = crypto.sign(data).unwrap();
        let verifying_key = crypto.verifying_key();

        assert!(crypto.verify(data, &signature, &verifying_key).unwrap());
    }
}
