use anyhow::Context;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use rand::{rngs::OsRng, RngCore};

#[derive(Clone)]
pub struct SecretCipher {
    cipher: XChaCha20Poly1305,
}

impl SecretCipher {
    pub fn from_key(key: [u8; 32]) -> Self {
        Self {
            cipher: XChaCha20Poly1305::new_from_slice(&key)
                .expect("fixed XChaCha20Poly1305 key length"),
        }
    }

    pub fn from_environment() -> anyhow::Result<Option<Self>> {
        let Some(value) = std::env::var("MIRRORPROXY_MASTER_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(None);
        };
        let key = URL_SAFE_NO_PAD
            .decode(value.trim())
            .context("MIRRORPROXY_MASTER_KEY must be unpadded base64url")?;
        if key.len() != 32 {
            anyhow::bail!("MIRRORPROXY_MASTER_KEY must decode to exactly 32 bytes");
        }
        let key: [u8; 32] = key.try_into().expect("validated master key length");
        Ok(Some(Self::from_key(key)))
    }

    pub fn encrypt(&self, purpose: &str, plaintext: &[u8]) -> anyhow::Result<String> {
        let mut nonce_bytes = [0_u8; 24];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from(nonce_bytes);
        let ciphertext = self
            .cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad: purpose.as_bytes(),
                },
            )
            .map_err(|_| anyhow::anyhow!("failed to encrypt secret"))?;
        let mut encoded = nonce_bytes.to_vec();
        encoded.extend_from_slice(&ciphertext);
        Ok(URL_SAFE_NO_PAD.encode(encoded))
    }

    pub fn decrypt(&self, purpose: &str, value: &str) -> anyhow::Result<Vec<u8>> {
        let encoded = URL_SAFE_NO_PAD
            .decode(value)
            .context("encrypted secret is not valid base64url")?;
        if encoded.len() <= 24 {
            anyhow::bail!("encrypted secret is truncated");
        }
        let nonce_bytes: [u8; 24] = encoded[..24]
            .try_into()
            .expect("validated encrypted nonce length");
        let nonce = XNonce::from(nonce_bytes);
        self.cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: &encoded[24..],
                    aad: purpose.as_bytes(),
                },
            )
            .map_err(|_| anyhow::anyhow!("failed to decrypt secret"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_values_are_randomized_and_bound_to_their_purpose() {
        let cipher = SecretCipher::from_key([7_u8; 32]);
        let first = cipher.encrypt("smtp-password", b"secret").unwrap();
        let second = cipher.encrypt("smtp-password", b"secret").unwrap();
        assert_ne!(first, second);
        assert_eq!(cipher.decrypt("smtp-password", &first).unwrap(), b"secret");
        assert!(cipher.decrypt("oauth-secret", &first).is_err());
    }
}
