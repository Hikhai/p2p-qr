use anyhow::Result;
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;

// Dummy crypto context using base64 (NOT secure). Replace with real AEAD + KDF later.
pub struct CryptoCtx;

impl CryptoCtx {
    pub fn new_dummy() -> Self { CryptoCtx }
    pub fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>> {
        Ok(STANDARD.encode(plain).into_bytes())
    }
    pub fn decrypt(&self, enc: &[u8]) -> Result<Vec<u8>> {
        let s = String::from_utf8_lossy(enc);
        let decoded = STANDARD.decode(&*s)?;
        Ok(decoded)
    }
}
