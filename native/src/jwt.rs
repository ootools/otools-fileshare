//! HS256 token，签名密钥仅驻留本次插件进程。

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde_json::{json, Value};
use sha2::Sha256;

use crate::account;

type HmacSha256 = Hmac<Sha256>;
static SECRET: OnceLock<[u8; 32]> = OnceLock::new();

fn secret() -> &'static [u8; 32] {
    SECRET.get_or_init(|| {
        let mut bytes = [0; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        bytes
    })
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn sign(mut payload: Value, expires_in_sec: i64) -> String {
    payload["iat"] = json!(now());
    if expires_in_sec > 0 {
        payload["exp"] = json!(now().saturating_add(expires_in_sec as u64));
    }
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
    let body =
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).expect("JSON value is serializable"));
    let input = format!("{header}.{body}");
    let mut mac = HmacSha256::new_from_slice(secret()).expect("HS256 key length");
    mac.update(input.as_bytes());
    format!(
        "{input}.{}",
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    )
}

pub fn verify(token: &str) -> Option<Value> {
    if token.len() > 16 * 1024 {
        return None;
    }
    let mut parts = token.split('.');
    let header = parts.next()?;
    let body = parts.next()?;
    let signature = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let header_value: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(header).ok()?).ok()?;
    if header_value["alg"] != "HS256" {
        return None;
    }
    let mut mac = HmacSha256::new_from_slice(secret()).ok()?;
    mac.update(format!("{header}.{body}").as_bytes());
    mac.verify_slice(&URL_SAFE_NO_PAD.decode(signature).ok()?)
        .ok()?;
    let payload: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(body).ok()?).ok()?;
    if !payload.is_object() {
        return None;
    }
    if let Some(exp) = payload.get("exp") {
        if exp.as_u64()? <= now() {
            return None;
        }
    }
    Some(payload)
}

pub fn admin_permissions() -> Value {
    let mut permissions = account::default_permissions();
    permissions["admin"] = json!(true);
    permissions
}

pub fn resolve_permissions(payload: &Value) -> Result<Option<Value>, String> {
    if let Some(username) = payload["username"].as_str() {
        return Ok(account::get_account_by_username(username)?
            .map(|account| account["permissions"].clone()));
    }
    Ok(payload
        .get("permissions")
        .filter(|permissions| permissions.is_object())
        .cloned())
}

#[cfg(test)]
mod tests {
    use crate::store::test_support::with_test_home;
    use serde_json::json;

    #[test]
    fn jwt_roundtrip_rejects_tampering_expiration_and_wrong_algorithm() {
        let token = super::sign(json!({"username":"alice"}), 3600);
        assert_eq!(super::verify(&token).unwrap()["username"], "alice");
        let mut bad = token.clone();
        bad.push('x');
        assert!(super::verify(&bad).is_none());
        assert!(super::verify("a.b.c").is_none());
        let expired = super::sign(json!({"exp":1}), 0);
        assert!(super::verify(&expired).is_none());
        let permanent = super::sign(json!({"permissions":{"download":true}}), 0);
        assert!(super::verify(&permanent).unwrap().get("exp").is_none());
    }

    #[test]
    fn tokens_use_live_account_permissions_and_deleted_accounts_are_invalid() {
        with_test_home(|_| {
            let created =
                crate::account::add_account(&json!({"username":"alice","password":"secret"}))
                    .unwrap();
            let id = created["data"]["id"].as_str().unwrap();
            let token = super::sign(
                json!({"username":"alice","permissions":{"download":true}}),
                0,
            );
            crate::account::update_account(id, &json!({"permissions":{"download":false}})).unwrap();
            assert_eq!(
                super::resolve_permissions(&super::verify(&token).unwrap())
                    .unwrap()
                    .unwrap()["download"],
                false
            );
            crate::account::remove_account(id).unwrap();
            assert!(super::resolve_permissions(&super::verify(&token).unwrap())
                .unwrap()
                .is_none());
        });
    }
}
