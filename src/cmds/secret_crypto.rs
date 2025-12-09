use crate::errors::KamError;
use hmac::{Hmac, Mac};
use openssl::{
    hash::MessageDigest,
    pkcs5::pbkdf2_hmac,
    rand::rand_bytes,
    symm::{Cipher, Crypter, Mode},
};
use sha2::Sha256;
// use rand::RngCore; // not used

const MAGIC: &[u8] = b"KAMKEYv1"; // 8 bytes
const SALT_LEN: usize = 16;
const IV_LEN: usize = 16; // AES-256-CBC uses 16 byte IV
const HMAC_LEN: usize = 32; // SHA256
const PBKDF2_ITERS: u32 = 100_000;

pub fn encrypt_with_password(plaintext: &[u8], password: &str) -> Result<Vec<u8>, KamError> {
    let mut salt = [0u8; SALT_LEN];
    rand_bytes(&mut salt).map_err(|e| KamError::CommandFailed(format!("Random failed: {}", e)))?;
    // derive 64 bytes key material
    let mut key_material = vec![0u8; 64];
    pbkdf2_hmac(
        password.as_bytes(),
        &salt,
        PBKDF2_ITERS.try_into().unwrap(),
        MessageDigest::sha256(),
        &mut key_material,
    )
    .map_err(|e| KamError::CommandFailed(format!("KDF failed: {}", e)))?;
    let key_enc = &key_material[0..32];
    let key_hmac = &key_material[32..64];

    // generate iv
    let mut iv = [0u8; IV_LEN];
    rand_bytes(&mut iv).map_err(|e| KamError::CommandFailed(format!("Random failed: {}", e)))?;

    // encrypt using AES-256-CBC
    let cipher = Cipher::aes_256_cbc();
    let mut crypter = Crypter::new(cipher, Mode::Encrypt, key_enc, Some(&iv))
        .map_err(|e| KamError::CommandFailed(format!("Crypter new failed: {}", e)))?;
    let mut out = vec![0u8; plaintext.len() + cipher.block_size()];
    let mut count = crypter
        .update(plaintext, &mut out)
        .map_err(|e| KamError::CommandFailed(format!("Encrypt update: {}", e)))?;
    count += crypter
        .finalize(&mut out[count..])
        .map_err(|e| KamError::CommandFailed(format!("Encrypt finalize: {}", e)))?;
    out.truncate(count);

    // compute HMAC: HMAC-SHA256(salt || iv || ciphertext)
    let mut mac = Hmac::<Sha256>::new_from_slice(key_hmac)
        .map_err(|e| KamError::CommandFailed(format!("HMAC new: {}", e)))?;
    mac.update(&salt);
    mac.update(&iv);
    mac.update(&out);
    let tag = mac.finalize().into_bytes();

    // Build final blob: MAGIC + salt + iv + ciphertext + tag
    let mut blob = Vec::new();
    blob.extend_from_slice(MAGIC);
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&iv);
    blob.extend_from_slice(&out);
    blob.extend_from_slice(&tag);
    Ok(blob)
}

pub fn decrypt_with_password(blob: &[u8], password: &str) -> Result<Vec<u8>, KamError> {
    // Validate header
    if blob.len() < MAGIC.len() + SALT_LEN + IV_LEN + HMAC_LEN + 1 {
        return Err(KamError::CommandFailed("Invalid blob length".to_string()));
    }
    if &blob[0..MAGIC.len()] != MAGIC {
        return Err(KamError::CommandFailed("Invalid magic header".to_string()));
    }
    let mut offset = MAGIC.len();
    let salt = &blob[offset..offset + SALT_LEN];
    offset += SALT_LEN;
    let iv = &blob[offset..offset + IV_LEN];
    offset += IV_LEN;
    let tag = &blob[blob.len() - HMAC_LEN..];
    let ciphertext = &blob[offset..(blob.len() - HMAC_LEN)];

    // derive keys
    let mut key_material = vec![0u8; 64];
    pbkdf2_hmac(
        password.as_bytes(),
        salt,
        PBKDF2_ITERS.try_into().unwrap(),
        MessageDigest::sha256(),
        &mut key_material,
    )
    .map_err(|e| KamError::CommandFailed(format!("KDF failed: {}", e)))?;
    let key_enc = &key_material[0..32];
    let key_hmac = &key_material[32..64];

    // verify HMAC
    let mut mac = Hmac::<Sha256>::new_from_slice(key_hmac)
        .map_err(|e| KamError::CommandFailed(format!("HMAC new failed: {}", e)))?;
    mac.update(salt);
    mac.update(iv);
    mac.update(ciphertext);
    mac.verify_slice(tag)
        .map_err(|_| KamError::CommandFailed("Invalid password or tampered blob".to_string()))?;

    // decrypt
    let cipher = Cipher::aes_256_cbc();
    let mut crypter = Crypter::new(cipher, Mode::Decrypt, key_enc, Some(iv))
        .map_err(|e| KamError::CommandFailed(format!("Crypter new: {}", e)))?;
    let mut out = vec![0u8; ciphertext.len() + cipher.block_size()];
    let mut count = crypter
        .update(ciphertext, &mut out)
        .map_err(|e| KamError::CommandFailed(format!("Decrypt update: {}", e)))?;
    count += crypter
        .finalize(&mut out[count..])
        .map_err(|e| KamError::CommandFailed(format!("Decrypt finalize: {}", e)))?;
    out.truncate(count);
    Ok(out)
}
