//! API Key 加密管理。
//!
//! 流程：
//!   主密钥派生(argon2: 机器ID + 可选密码 + 固定盐)
//!     → AES-256-GCM 加密 API Key
//!     → 存 SQLite (ciphertext + nonce)
//!   使用时：读取 → 解密 → 临时持有 → 调用 API → zeroize
//!
//! 主密钥不落盘；前端永不接触明文 Key。

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::Argon2;
use rand::RngCore;
use zeroize::{Zeroize, Zeroizing};

use ripple_core::RippleError;

/// AES-GCM nonce 固定 12 字节
const NONCE_LEN: usize = 12;
/// AES-256 密钥 32 字节
const KEY_LEN: usize = 32;
/// 固定应用盐（per-app；不同应用应不同）
const APP_SALT: &[u8] = b"ripple-app-v1-key-derivation-salt";

/// 加密后的密钥载荷（存数据库用）
#[derive(Debug, Clone, zeroize::Zeroize)]
pub struct EncryptedKey {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
}

/// API Key 加密器。
///
/// 持有派生出的主密钥（`Zeroizing` 包裹，drop 时安全清零）。
/// AES cipher 不持久持有——每次加解密临时构造，避免密钥驻留。
pub struct KeyManager {
    master_key: Zeroizing<[u8; KEY_LEN]>,
}

#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("key derivation failed")]
    KeyDerivation,
    #[error("cipher init failed")]
    CipherInit,
    #[error("encryption failed")]
    Encryption,
    #[error("decryption failed (wrong key or corrupted data)")]
    Decryption,
    #[error("invalid utf8 after decrypt")]
    InvalidUtf8,
}

impl From<SecurityError> for RippleError {
    fn from(e: SecurityError) -> Self {
        RippleError::Security(e.to_string())
    }
}

impl KeyManager {
    /// 用机器标识 + 可选用户密码派生主密钥。
    ///
    /// - `machine_id`：机器唯一标识（如安装时生成的随机 UUID）
    /// - `user_password`：可选主密码；为 None 则仅靠 machine_id 保护
    pub fn new(machine_id: &str, user_password: Option<&str>) -> Result<Self, SecurityError> {
        let mut key_material = machine_id.as_bytes().to_vec();
        if let Some(pw) = user_password {
            key_material.extend_from_slice(pw.as_bytes());
        }

        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        Argon2::default()
            .hash_password_into(&key_material, APP_SALT, &mut *key)
            .map_err(|_| SecurityError::KeyDerivation)?;

        key_material.zeroize();

        Ok(Self { master_key: key })
    }

    /// 加密一段明文（如 API Key），返回 (ciphertext, nonce)
    pub fn encrypt(&self, plaintext: &str) -> Result<EncryptedKey, SecurityError> {
        let cipher = Aes256Gcm::new_from_slice(&*self.master_key)
            .map_err(|_| SecurityError::CipherInit)?;

        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|_| SecurityError::Encryption)?;

        Ok(EncryptedKey {
            ciphertext,
            nonce: nonce_bytes.to_vec(),
        })
    }

    /// 解密
    pub fn decrypt(&self, encrypted: &EncryptedKey) -> Result<String, SecurityError> {
        let cipher = Aes256Gcm::new_from_slice(&*self.master_key)
            .map_err(|_| SecurityError::CipherInit)?;

        if encrypted.nonce.len() != NONCE_LEN {
            return Err(SecurityError::Decryption);
        }
        let nonce = Nonce::from_slice(&encrypted.nonce);
        let plaintext = cipher
            .decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|_| SecurityError::Decryption)?;

        String::from_utf8(plaintext).map_err(|_| SecurityError::InvalidUtf8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mgr() -> KeyManager {
        KeyManager::new("test-machine-id", Some("password")).unwrap()
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let m = mgr();
        let key = "sk-abcdefghij1234567890";
        let enc = m.encrypt(key).unwrap();
        let dec = m.decrypt(&enc).unwrap();
        assert_eq!(dec, key);
    }

    #[test]
    fn different_nonce_each_encrypt() {
        let m = mgr();
        let e1 = m.encrypt("same-key").unwrap();
        let e2 = m.encrypt("same-key").unwrap();
        // 相同明文，nonce 不同 → 密文不同
        assert_ne!(e1.nonce, e2.nonce);
        assert_ne!(e1.ciphertext, e2.ciphertext);
    }

    #[test]
    fn wrong_password_fails_decrypt() {
        let m1 = KeyManager::new("machine", Some("pass1")).unwrap();
        let m2 = KeyManager::new("machine", Some("pass2")).unwrap();
        let enc = m1.encrypt("secret").unwrap();
        assert!(m2.decrypt(&enc).is_err());
    }

    #[test]
    fn wrong_machine_fails_decrypt() {
        let m1 = KeyManager::new("machine-a", None).unwrap();
        let m2 = KeyManager::new("machine-b", None).unwrap();
        let enc = m1.encrypt("secret").unwrap();
        assert!(m2.decrypt(&enc).is_err());
    }

    #[test]
    fn corrupted_ciphertext_fails() {
        let m = mgr();
        let mut enc = m.encrypt("secret").unwrap();
        enc.ciphertext[0] ^= 0xff; // 篡改
        assert!(m.decrypt(&enc).is_err());
    }

    #[test]
    fn empty_plaintext_works() {
        let m = mgr();
        let enc = m.encrypt("").unwrap();
        assert_eq!(m.decrypt(&enc).unwrap(), "");
    }

    #[test]
    fn unicode_plaintext_roundtrip() {
        let m = mgr();
        let key = "密钥-🔑-äöü";
        let enc = m.encrypt(key).unwrap();
        assert_eq!(m.decrypt(&enc).unwrap(), key);
    }
}
