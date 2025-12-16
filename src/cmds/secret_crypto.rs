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

// 加密格式的魔数（用于识别加密数据）
const MAGIC: &[u8] = b"KAMKEYv1"; // 8字节
const SALT_LEN: usize = 16;
const IV_LEN: usize = 16; // AES-256-CBC用16字节IV
const HMAC_LEN: usize = 32; // SHA256的HMAC是32字节
const PBKDF2_ITERS: u32 = 100_000; // PBKDF2迭代次数，虽然可能有点慢但安全第一

// 用密码加密数据
// 使用AES-256-CBC加密，HMAC-SHA256验证
pub fn encrypt_with_password(plaintext: &[u8], password: &str) -> Result<Vec<u8>, KamError> {
    // 生成随机salt
    let mut salt = [0u8; SALT_LEN];
    rand_bytes(&mut salt).map_err(|e| KamError::CommandFailed(format!("Random failed: {}", e)))?;
    // 用PBKDF2派生64字节密钥材料（32字节加密密钥 + 32字节HMAC密钥）
    let mut key_material = vec![0u8; 64];
    pbkdf2_hmac(
        password.as_bytes(),
        &salt,
        PBKDF2_ITERS.try_into().unwrap(),
        MessageDigest::sha256(),
        &mut key_material,
    )
    .map_err(|e| KamError::CommandFailed(format!("KDF failed: {}", e)))?;
    let key_enc = &key_material[0..32]; // 前32字节用于加密
    let key_hmac = &key_material[32..64]; // 后32字节用于HMAC

    // 生成随机IV
    let mut iv = [0u8; IV_LEN];
    rand_bytes(&mut iv).map_err(|e| KamError::CommandFailed(format!("Random failed: {}", e)))?;

    // 用AES-256-CBC加密
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

    // 计算HMAC：HMAC-SHA256(salt || iv || ciphertext)
    // 用于验证数据完整性
    let mut mac = Hmac::<Sha256>::new_from_slice(key_hmac)
        .map_err(|e| KamError::CommandFailed(format!("HMAC new: {}", e)))?;
    mac.update(&salt);
    mac.update(&iv);
    mac.update(&out);
    let tag = mac.finalize().into_bytes();

    // 构建最终的blob：MAGIC + salt + iv + ciphertext + tag
    // 这样解密时能知道格式和验证完整性
    let mut blob = Vec::new();
    blob.extend_from_slice(MAGIC);
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&iv);
    blob.extend_from_slice(&out);
    blob.extend_from_slice(&tag);
    Ok(blob)
}

// 用密码解密数据
// 先验证HMAC，再解密
pub fn decrypt_with_password(blob: &[u8], password: &str) -> Result<Vec<u8>, KamError> {
    // 验证头部和长度
    if blob.len() < MAGIC.len() + SALT_LEN + IV_LEN + HMAC_LEN + 1 {
        return Err(KamError::CommandFailed("Invalid blob length".to_string()));
    }
    if &blob[0..MAGIC.len()] != MAGIC {
        return Err(KamError::CommandFailed("Invalid magic header".to_string()));
    }
    // 提取各个部分：salt、iv、ciphertext、tag
    let mut offset = MAGIC.len();
    let salt = &blob[offset..offset + SALT_LEN];
    offset += SALT_LEN;
    let iv = &blob[offset..offset + IV_LEN];
    offset += IV_LEN;
    let tag = &blob[blob.len() - HMAC_LEN..]; // tag在最后
    let ciphertext = &blob[offset..(blob.len() - HMAC_LEN)];

    // 派生密钥（和加密时一样）
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

    // 验证HMAC（确保数据没被篡改）
    let mut mac = Hmac::<Sha256>::new_from_slice(key_hmac)
        .map_err(|e| KamError::CommandFailed(format!("HMAC new failed: {}", e)))?;
    mac.update(salt);
    mac.update(iv);
    mac.update(ciphertext);
    // 验证HMAC，失败说明密码错误或数据被篡改
    mac.verify_slice(tag)
        .map_err(|_| KamError::CommandFailed("Invalid password or tampered blob".to_string()))?;

    // HMAC验证通过，可以解密了
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
    // 解密完成！虽然过程有点复杂，但至少安全
}
