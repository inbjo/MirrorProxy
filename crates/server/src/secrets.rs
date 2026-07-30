use anyhow::Context;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use rand::{rngs::OsRng, RngCore};

const PREFIX: &str = "enc:v1:";
const NONCE_LEN: usize = 24;

#[derive(Clone)]
pub struct SecretCipher {
    cipher: Option<XChaCha20Poly1305>,
}

impl SecretCipher {
    pub fn from_env() -> anyhow::Result<Self> {
        let Some(encoded) = std::env::var("MIRRORPROXY_MASTER_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(Self { cipher: None });
        };
        let bytes = decode_master_key(encoded.trim())?;
        Ok(Self {
            cipher: Some(XChaCha20Poly1305::new(Key::from_slice(&bytes))),
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.cipher.is_some()
    }

    #[cfg(test)]
    pub(crate) fn from_test_key(key: [u8; 32]) -> Self {
        Self {
            cipher: Some(XChaCha20Poly1305::new(Key::from_slice(&key))),
        }
    }

    pub fn seal(&self, context: &str, value: &str) -> anyhow::Result<String> {
        if value.is_empty() {
            return Ok(value.to_string());
        }
        if value.starts_with(PREFIX) {
            self.open(context, value)?;
            return Ok(value.to_string());
        }
        if self.cipher.is_none() {
            return Ok(value.to_string());
        }
        let mut nonce = [0_u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = self
            .cipher
            .as_ref()
            .expect("checked cipher presence")
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: value.as_bytes(),
                    aad: context.as_bytes(),
                },
            )
            .map_err(|_| anyhow::anyhow!("failed to encrypt {context}"))?;
        let mut payload = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        payload.extend_from_slice(&nonce);
        payload.extend_from_slice(&ciphertext);
        Ok(format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(payload)))
    }

    pub fn open(&self, context: &str, value: &str) -> anyhow::Result<String> {
        let Some(encoded) = value.strip_prefix(PREFIX) else {
            return Ok(value.to_string());
        };
        let cipher = self.cipher.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "encrypted {context} requires the MIRRORPROXY_MASTER_KEY used when it was stored"
            )
        })?;
        let payload = URL_SAFE_NO_PAD
            .decode(encoded)
            .with_context(|| format!("stored {context} ciphertext is invalid"))?;
        if payload.len() <= NONCE_LEN {
            anyhow::bail!("stored {context} ciphertext is truncated");
        }
        let plaintext = cipher
            .decrypt(
                XNonce::from_slice(&payload[..NONCE_LEN]),
                Payload {
                    msg: &payload[NONCE_LEN..],
                    aad: context.as_bytes(),
                },
            )
            .map_err(|_| anyhow::anyhow!("failed to decrypt {context}; master key is incorrect"))?;
        String::from_utf8(plaintext).with_context(|| format!("decrypted {context} is not UTF-8"))
    }

    pub fn seal_optional(
        &self,
        context: &str,
        value: Option<&str>,
    ) -> anyhow::Result<Option<String>> {
        value.map(|value| self.seal(context, value)).transpose()
    }

    pub fn open_optional(
        &self,
        context: &str,
        value: Option<String>,
    ) -> anyhow::Result<Option<String>> {
        value.map(|value| self.open(context, &value)).transpose()
    }
}

fn decode_master_key(encoded: &str) -> anyhow::Result<Vec<u8>> {
    let bytes = if encoded.len() == 64 && encoded.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        hex::decode(encoded)
            .context("MIRRORPROXY_MASTER_KEY contains invalid hexadecimal characters")?
    } else {
        URL_SAFE_NO_PAD
            .decode(encoded)
            .context("MIRRORPROXY_MASTER_KEY must be 32 bytes encoded as base64url or hex")?
    };
    if bytes.len() != 32 {
        anyhow::bail!("MIRRORPROXY_MASTER_KEY must decode to exactly 32 bytes");
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cipher() -> SecretCipher {
        SecretCipher {
            cipher: Some(XChaCha20Poly1305::new(Key::from_slice(&[7_u8; 32]))),
        }
    }

    #[test]
    fn round_trip_binds_ciphertext_to_its_storage_context() {
        let cipher = cipher();
        let encrypted = cipher.seal("smtp.password", "secret").unwrap();
        assert!(encrypted.starts_with(PREFIX));
        assert_eq!(cipher.open("smtp.password", &encrypted).unwrap(), "secret");
        assert!(cipher.open("oauth.client_secret", &encrypted).is_err());
    }

    #[test]
    fn disabled_cipher_preserves_plaintext_for_compatible_local_deployments() {
        let cipher = SecretCipher { cipher: None };
        assert_eq!(cipher.seal("smtp.password", "secret").unwrap(), "secret");
        assert_eq!(cipher.open("smtp.password", "legacy").unwrap(), "legacy");
        assert!(cipher.open("smtp.password", "enc:v1:invalid").is_err());
    }

    #[test]
    fn accepts_unambiguous_hex_and_base64url_master_keys() {
        assert_eq!(
            decode_master_key("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
                .unwrap()
                .len(),
            32
        );
        assert_eq!(
            decode_master_key("AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8")
                .unwrap()
                .len(),
            32
        );
    }
}
