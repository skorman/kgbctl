//! API bearer tokens. A token is 256 random bits, so a fast hash is enough to
//! keep the stored copy useless to anyone who reads the database.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

const PREFIX: &str = "kgb_";

/// A fresh token, e.g. `kgb_3q2-7wE...`. Show it once; store only `hash(token)`.
pub fn generate() -> anyhow::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("generating token: {e}"))?;
    Ok(format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes)))
}

/// Hex SHA-256 of the token, as stored in `api_users.token_hash`.
pub fn hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_unique_and_hash_stably() {
        let a = generate().unwrap();
        let b = generate().unwrap();
        assert_ne!(a, b);
        assert!(a.starts_with(PREFIX));
        assert_eq!(a.len(), PREFIX.len() + 43);

        assert_eq!(hash(&a), hash(&a));
        assert_ne!(hash(&a), hash(&b));
        assert_eq!(
            hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
