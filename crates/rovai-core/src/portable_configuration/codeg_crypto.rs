//! 固定 Codeg config_sync/crypto.rs 的 envelope1 兼容读取；不改变原加密算法。
use super::*;
use argon2::{Algorithm,Argon2,Params,Version};
use base64::{Engine,engine::general_purpose::STANDARD as B64};

#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct KdfParams{m_cost:u32,t_cost:u32,p_cost:u32,version:u32}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct EncryptedPayload{
    codeg_config_encryption:u32,algo:String,kdf:String,kdf_params:KdfParams,
    salt_b64:String,nonce_b64:String,ciphertext_b64:String,
}
pub(super) fn decrypt(value:Value,passphrase:&str)->Result<Vec<u8>>{
    let payload:EncryptedPayload=serde_json::from_value(value).map_err(|_|anyhow::anyhow!("configuration.encrypted_header_invalid"))?;
    ensure!(payload.codeg_config_encryption==1&&payload.algo.eq_ignore_ascii_case("AES-256-GCM")&&payload.kdf.eq_ignore_ascii_case("Argon2id"),"configuration.encrypted_format_unsupported");
    ensure!(!passphrase.is_empty()&&passphrase.len()<=16000,"configuration.passphrase_required");
    let params=&payload.kdf_params;
    ensure!((8..=256*1024).contains(&params.m_cost)&&(1..=10).contains(&params.t_cost)&&(1..=4).contains(&params.p_cost)&&matches!(params.version,0x10|0x13),"configuration.encrypted_kdf_cost_invalid");
    ensure!(payload.salt_b64.len()<=88&&payload.nonce_b64.len()<=24&&payload.ciphertext_b64.len()<=MAX_ARCHIVE,"configuration.encrypted_header_invalid");
    let salt=B64.decode(payload.salt_b64).map_err(|_|anyhow::anyhow!("configuration.encrypted_salt_invalid"))?;
    let nonce=B64.decode(payload.nonce_b64).map_err(|_|anyhow::anyhow!("configuration.encrypted_nonce_invalid"))?;
    ensure!((8..=64).contains(&salt.len())&&nonce.len()==12,"configuration.encrypted_header_invalid");
    let mut cipher=B64.decode(payload.ciphertext_b64).map_err(|_|anyhow::anyhow!("configuration.encrypted_content_invalid"))?;
    ensure!(cipher.len()>=16&&cipher.len()<=MAX_ARCHIVE,"configuration.encrypted_content_invalid");
    let parameters=Params::new(params.m_cost,params.t_cost,params.p_cost,Some(32)).map_err(|_|anyhow::anyhow!("configuration.encrypted_kdf_cost_invalid"))?;
    let mut key=[0u8;32];
    Argon2::new(Algorithm::Argon2id,if params.version==0x10{Version::V0x10}else{Version::V0x13},parameters).hash_password_into(passphrase.as_bytes(),&salt,&mut key).map_err(|_|anyhow::anyhow!("configuration.key_derivation_failed"))?;
    let secret=ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM,&key).map_err(|_|anyhow::anyhow!("configuration.cipher_unavailable"))?;
    key.fill(0);
    let plain=ring::aead::LessSafeKey::new(secret).open_in_place(ring::aead::Nonce::try_assume_unique_for_key(&nonce).map_err(|_|anyhow::anyhow!("configuration.encrypted_nonce_invalid"))?,ring::aead::Aad::empty(),&mut cipher).map_err(|_|anyhow::anyhow!("configuration.passphrase_or_content_invalid"))?;
    ensure!(plain.len()<=MAX_ARCHIVE,"configuration.archive_too_large");Ok(plain.to_vec())
}
