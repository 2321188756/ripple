//! ripple-security: API Key 加密与运行时安全。
//!
//! AES-256-GCM 加密 + argon2 密钥派生 + zeroize 内存清零。

pub mod key_manager;

pub use key_manager::{EncryptedKey, KeyManager, SecurityError};
