use super::*;
use data_encoding::BASE64URL_NOPAD;

pub async fn save_user_secret(namespace: &str, key: &str, value: &[u8]) -> Result<bool, String> {
    let mut s = BASE64URL_NOPAD.encode(value);
    s.push('!');

    save_user_secret_string(namespace, key, s.as_str()).await
}

pub async fn load_user_secret(namespace: &str, key: &str) -> Result<Option<Vec<u8>>, String> {
    let mut s = match load_user_secret_string(namespace, key).await? {
        Some(s) => s,
        None => {
            return Ok(None);
        }
    };

    if s.pop() != Some('!') {
        return Err("User secret is not a buffer".to_owned());
    }

    let mut bytes = Vec::<u8>::new();
    let res = BASE64URL_NOPAD.decode_len(s.len());
    match res {
        Ok(l) => {
            bytes.resize(l, 0u8);
        }
        Err(_) => {
            return Err("Failed to decode".to_owned());
        }
    }

    let res = BASE64URL_NOPAD.decode_mut(s.as_bytes(), &mut bytes);
    match res {
        Ok(_) => Ok(Some(bytes)),
        Err(_) => Err("Failed to decode".to_owned()),
    }
}

pub async fn remove_user_secret(namespace: &str, key: &str) -> Result<bool, String> {
    remove_user_secret_string(namespace, key).await
}
