# ripple-security

API Key AES-256-GCM 加密存储。

## 实现

- `KeyManager`：argon2 密钥派生 + AES-256-GCM 加密/解密
- 主密钥 Zeroizing 包裹，drop 时安全清零
- 加密载荷：ciphertext + nonce（12 字节随机 nonce，每次不同）

## 安全原则

- 解密后 Key 仅在单次 API 调用期间存活于内存
- 前端永不接触明文 Key
- 主密钥不持久化

## 测试（7 个）

- encrypt_decrypt_roundtrip / different_nonce_each_encrypt
- wrong_password_fails_decrypt / wrong_machine_fails_decrypt
- corrupted_ciphertext_fails / empty_plaintext_works / unicode_plaintext_roundtrip
