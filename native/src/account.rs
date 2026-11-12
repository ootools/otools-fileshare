//! 本地账号和路径权限；权限编辑即时生效。

use regex::RegexBuilder;
use serde_json::{json, Value};

use crate::store;

const ACCOUNTS_KEY: &str = "accounts";

pub fn default_permissions() -> Value {
    json!({"download":true, "uploadFile":true, "uploadText":true, "allowAccess":"", "denyAccess":""})
}

pub fn normalize_permissions(permissions: &Value) -> Value {
    let mut base = default_permissions();
    for key in ["download", "uploadFile", "uploadText"] {
        if let Some(value) = permissions.get(key).filter(|value| !value.is_null()) {
            base[key] = json!(match value {
                Value::Bool(v) => *v,
                Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
                Value::String(s) => !s.is_empty(),
                _ => true,
            });
        }
    }
    for key in ["allowAccess", "denyAccess"] {
        if let Some(value) = permissions.get(key).and_then(Value::as_str) {
            base[key] = json!(value);
        }
    }
    base
}

fn accounts(value: &mut Value) -> Result<&mut Vec<Value>, String> {
    if let Value::String(encoded) = value {
        *value = serde_json::from_str(encoded).map_err(|error| error.to_string())?;
    }
    value
        .as_array_mut()
        .ok_or_else(|| "账号数据格式错误".to_string())
}

fn normalized(account: &Value) -> Value {
    let mut account = account.clone();
    account["permissions"] = normalize_permissions(&account["permissions"]);
    account
}

pub fn list_accounts() -> Result<Vec<Value>, String> {
    let mut value = store::get_item_with_default(ACCOUNTS_KEY, json!([]))?;
    Ok(accounts(&mut value)?.iter().map(normalized).collect())
}

pub fn get_account_by_username(username: &str) -> Result<Option<Value>, String> {
    Ok(list_accounts()?
        .into_iter()
        .find(|account| account["username"].as_str() == Some(username)))
}

pub fn add_account(input: &Value) -> Result<Value, String> {
    let username = input["username"].as_str().unwrap_or_default().trim();
    let password = input["password"].as_str().unwrap_or_default();
    if username.is_empty() {
        return Err("用户名不能为空".to_string());
    }
    if password.is_empty() {
        return Err("密码不能为空".to_string());
    }
    store::mutate_item(ACCOUNTS_KEY, json!([]), |value| {
        let accounts = accounts(value)?;
        if accounts
            .iter()
            .any(|account| account["username"].as_str() == Some(username))
        {
            return Err("用户名已存在".to_string());
        }
        let account = json!({"id":&uuid::Uuid::new_v4().simple().to_string()[..16], "username":username, "password":password, "permissions":normalize_permissions(&input["permissions"])});
        accounts.push(account.clone());
        Ok(json!({"success":true,"message":"添加成功","data":account}))
    })
}

pub fn update_account(id: &str, input: &Value) -> Result<Value, String> {
    store::mutate_item(ACCOUNTS_KEY, json!([]), |value| {
        let accounts = accounts(value)?;
        let index = accounts
            .iter()
            .position(|account| account["id"].as_str() == Some(id))
            .ok_or("账号不存在")?;
        let username = input["username"]
            .as_str()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| accounts[index]["username"].as_str().unwrap_or_default())
            .trim()
            .to_string();
        if username.is_empty() {
            return Err("用户名不能为空".to_string());
        }
        if accounts
            .iter()
            .enumerate()
            .any(|(i, a)| i != index && a["username"].as_str() == Some(&username))
        {
            return Err("用户名已存在".to_string());
        }
        accounts[index]["username"] = json!(username);
        if let Some(password) = input["password"]
            .as_str()
            .filter(|password| !password.is_empty())
        {
            accounts[index]["password"] = json!(password);
        }
        if input.get("permissions").is_some_and(Value::is_object) {
            accounts[index]["permissions"] = normalize_permissions(&input["permissions"]);
        }
        Ok(json!({"success":true,"message":"修改成功","data":normalized(&accounts[index])}))
    })
}

pub fn update_account_password(id: &str, password: &str) -> Result<Value, String> {
    if password.is_empty() {
        return Err("密码不能为空".to_string());
    }
    store::mutate_item(ACCOUNTS_KEY, json!([]), |value| {
        let account = accounts(value)?
            .iter_mut()
            .find(|account| account["id"].as_str() == Some(id))
            .ok_or("账号不存在")?;
        account["password"] = json!(password);
        Ok(json!({"success":true,"message":"修改成功"}))
    })
}

pub fn remove_account(id: &str) -> Result<Value, String> {
    store::mutate_item(ACCOUNTS_KEY, json!([]), |value| {
        let accounts = accounts(value)?;
        let index = accounts
            .iter()
            .position(|account| account["id"].as_str() == Some(id))
            .ok_or("账号不存在")?;
        accounts.remove(index);
        Ok(json!({"success":true,"message":"删除成功"}))
    })
}

pub fn match_rules(path: &str, rules: &str) -> bool {
    let path = path.replace('\\', "/");
    rules
        .split([',', '\n', '，'])
        .map(|rule| rule.trim().replace('\\', "/"))
        .any(|rule| {
            let rule = rule.trim_end_matches('/');
            if rule.is_empty() {
                return false;
            }
            let pattern = rule
                .split('*')
                .map(regex::escape)
                .collect::<Vec<_>>()
                .join(".*");
            RegexBuilder::new(&format!("^{pattern}"))
                .case_insensitive(true)
                .build()
                .is_ok_and(|regex| regex.is_match(&path))
        })
}

pub fn can_access_path(path: &str, permissions: &Value) -> bool {
    let permissions = normalize_permissions(permissions);
    let deny = permissions["denyAccess"].as_str().unwrap_or_default();
    let allow = permissions["allowAccess"].as_str().unwrap_or_default();
    !match_rules(path, deny) && (allow.is_empty() || match_rules(path, allow))
}

#[cfg(test)]
mod tests {
    use crate::store::test_support::with_test_home;
    use serde_json::json;

    #[test]
    fn account_validation_duplicate_names_and_empty_password_updates() {
        with_test_home(|_| {
            assert_eq!(
                super::add_account(&json!({})).unwrap_err(),
                "用户名不能为空"
            );
            assert_eq!(
                super::add_account(&json!({"username":"test"})).unwrap_err(),
                "密码不能为空"
            );
            let created =
                super::add_account(&json!({"username":" alice ","password":"secret"})).unwrap();
            let id = created["data"]["id"].as_str().unwrap();
            assert_eq!(id.len(), 16);
            assert_eq!(created["data"]["username"], "alice");
            assert_eq!(
                super::add_account(&json!({"username":"alice","password":"other"})).unwrap_err(),
                "用户名已存在"
            );
            super::update_account(
                id,
                &json!({"username":"alice","password":"","permissions":{"download":false}}),
            )
            .unwrap();
            let account = super::get_account_by_username("alice").unwrap().unwrap();
            assert_eq!(account["password"], "secret");
            assert_eq!(account["permissions"]["download"], false);
            assert_eq!(account["permissions"]["uploadFile"], true);
            super::update_account_password(id, "new-secret").unwrap();
            assert_eq!(
                super::get_account_by_username("alice").unwrap().unwrap()["password"],
                "new-secret"
            );
            super::remove_account(id).unwrap();
            assert!(super::get_account_by_username("alice").unwrap().is_none());
        });
    }

    #[test]
    fn access_rules_normalize_separators_and_deny_wins() {
        let permissions = json!({"allowAccess":"C:\\Projects，/home/*/public", "denyAccess":"C:\\Projects\\secret\n/home/ann/public/private"});
        assert!(super::can_access_path(
            "c:/PROJECTS/readme.txt",
            &permissions
        ));
        assert!(!super::can_access_path(
            "C:/Projects/secret/key.txt",
            &permissions
        ));
        assert!(super::can_access_path(
            "/home/ann/public/file.txt",
            &permissions
        ));
        assert!(!super::can_access_path(
            "/home/ann/public/private/key",
            &permissions
        ));
        assert!(!super::can_access_path("/outside/file", &permissions));
        assert!(super::can_access_path("/anything", &json!({})));
    }
}
