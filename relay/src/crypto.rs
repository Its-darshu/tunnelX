use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Hash a token using SHA256 for secure storage.
pub fn hash_token(token: &str) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Verify a token by comparing hashes.
pub fn verify_token(token: &str, stored_hash: &str) -> bool {
    let computed_hash = hash_token(token);
    computed_hash == stored_hash
}

/// Sign a message with a symmetric key using HMAC-SHA256.
pub fn sign_message(key: &[u8], message: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key)
        .expect("HMAC can take key of any size");
    mac.update(message);
    hex::encode(mac.finalize().into_bytes())
}

/// Verify a message signature using HMAC-SHA256.
pub fn verify_signature(key: &[u8], message: &[u8], signature: &str) -> bool {
    match hex::decode(signature) {
        Ok(sig_bytes) => {
            let mut mac = HmacSha256::new_from_slice(key)
                .expect("HMAC can take key of any size");
            mac.update(message);
            mac.verify_slice(&sig_bytes).is_ok()
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_hash_is_deterministic() {
        let token = "test-token-12345";
        let hash1 = hash_token(token);
        let hash2 = hash_token(token);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn verify_token_succeeds_with_correct_hash() {
        let token = "my-secret-token";
        let hash = hash_token(token);
        assert!(verify_token(token, &hash));
    }

    #[test]
    fn verify_token_fails_with_wrong_token() {
        let token = "correct-token";
        let hash = hash_token(token);
        assert!(!verify_token("wrong-token", &hash));
    }

    #[test]
    fn hmac_signature_is_verifiable() {
        let key = b"secret-key";
        let message = b"important-message";
        let signature = sign_message(key, message);
        assert!(verify_signature(key, message, &signature));
    }

    #[test]
    fn hmac_signature_fails_with_wrong_message() {
        let key = b"secret-key";
        let message = b"important-message";
        let signature = sign_message(key, message);
        assert!(!verify_signature(key, b"tampered-message", &signature));
    }

    #[test]
    fn hmac_signature_fails_with_wrong_key() {
        let key = b"secret-key";
        let message = b"important-message";
        let signature = sign_message(key, message);
        assert!(!verify_signature(b"wrong-key", message, &signature));
    }
}
