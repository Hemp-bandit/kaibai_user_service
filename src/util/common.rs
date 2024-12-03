use crate::{user::RedisLoginData, RB};
use chrono::{DateTime, Local, Utc};
use derive_more::derive::Display;
use lazy_regex::regex;
use rbatis::executor::RBatisTxExecutorGuard;
use rbatis::Error;
use redis::Commands;
use serde::Serialize;
use std::env::var;
use utoipa::{
    openapi::{
        self,
        security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    },
    Modify,
};

use hmac::{Hmac, Mac};
use jwt::{SignWithKey, VerifyWithKey};
use sha2::Sha256;

#[derive(Debug, Serialize)]
pub struct JWT;

impl Modify for JWT {
    fn modify(&self, openapi: &mut openapi::OpenApi) {
        if let Some(schema) = openapi.components.as_mut() {
            schema.add_security_scheme(
                "JWT",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}

#[derive(Debug, Display)]
pub enum RedisKeys {
    #[display("user_ids")]
    UserIds,

    #[display("user_info")]
    UserInfo,

    #[display("role_ids")]
    RoleIds,

    #[display("role_info")]
    RoleInfo,

    #[display("user_roles")]
    UserRoles,

    #[display("access_map")]
    AccessMap,

    #[display("access_map_ids")]
    AccessMapIds,
}

/**
 * 获取当前时间的时间戳
 * 格式: YYYY-MM-DD HH:mm:ss
 */
pub fn get_current_time_fmt() -> String {
    let dt = Utc::now();
    let local_dt: DateTime<Local> = dt.with_timezone(&Local);
    return local_dt.format("%Y-%m-%d %H:%M:%S").to_string();
}

/**
 * 获取当前时间戳
 * 秒
 */
pub fn get_current_timestamp() -> i64 {
    let dt = Utc::now();
    let local_dt: DateTime<Local> = dt.with_timezone(&Local);
    local_dt.timestamp()
}

/// 检测手机号是否合法
pub fn check_phone(phone: &str) -> bool {
    let max_len = 11;
    if phone.len() != max_len {
        log::error!("手机号长度不对");
        return false;
    }
    let r = regex!(r"^1[3-9]\d{9}$");

    r.is_match(phone)
}

pub async fn get_transaction_tx() -> Result<RBatisTxExecutorGuard, Error> {
    let tx = RB.acquire_begin().await.unwrap();
    let tx: RBatisTxExecutorGuard = tx.defer_async(|mut tx| async move {
        if tx.done {
            log::info!("transaction [{}] complete.", tx.tx_id);
        } else {
            let r = tx.rollback().await;
            if let Err(e) = r {
                log::error!("transaction [{}] rollback fail={}", tx.tx_id, e);
            } else {
                log::info!("transaction [{}] rollback", tx.tx_id);
            }
        }
    });
    log::info!("transaction [{}] start", tx.tx.as_ref().unwrap().tx_id);
    Ok(tx)
}

pub fn gen_access_value(bit: u64) -> u64 {
    let mod_val = bit % 31;
    let last_number = 1 << (mod_val.min(31) - 1);
    last_number
}

pub fn marge_access(arr: Vec<u64>) -> u64 {
    let mut res = 0;
    arr.into_iter().for_each(|val| {
        res += val;
    });
    res
}

pub fn has_access(auth: u64, access: Vec<u64>) -> bool {
    let mut res = false;
    access.into_iter().for_each(|val| {
        res = val & auth > 0;
    });
    res
}

pub fn gen_jwt_token(login_data: RedisLoginData) -> String {
    let jwt_secret =
        std::env::var("JWT_SECRET").unwrap_or("QWERTYUOas;ldfj;4u1023740^&&*()_)*&^".to_string());
    let key: Hmac<Sha256> = Hmac::new_from_slice(jwt_secret.as_bytes()).unwrap();
    let token_str = login_data.sign_with_key(&key).unwrap();

    token_str
}

pub fn jwt_token_to_data(jwt_token: String) -> Option<RedisLoginData> {
    log::debug!("jwt_token {jwt_token}");
    if jwt_token.is_empty() {
        return None;
    }
    let jwt_secret =
        var("JWT_SECRET").unwrap_or("QWERTYUOas;ldfj;4u1023740^&&*()_)*&^".to_string());
    let key: Hmac<Sha256> =
        Hmac::new_from_slice(jwt_secret.as_bytes()).expect("解析jwt token 失败");
    let claims: Result<RedisLoginData, jwt::Error> = jwt_token.verify_with_key(&key);
    match claims {
        Err(err) => {
            log::error!("{}", err.to_string());
            return None;
        }
        Ok(res) => Some(res),
    }
}

pub fn rds_str_to_list<T, U: Fn(String) -> T>(
    mut rds: std::sync::MutexGuard<'_, redis::Connection>,
    ids: Vec<i32>,
    key: RedisKeys,
    cb: U,
) -> Vec<T> {
    let mut res: Vec<T> = vec![];
    for id in ids {
        let data: Option<String> = rds.hget(key.to_string(), id).expect("asdf");
        if let Some(str) = data {
            let item: T = cb(str);
            res.push(item);
        }
    }
    res
}

#[cfg(test)]
mod test {

    use crate::{user::RedisLoginData, util::common::check_phone};

    use super::{gen_access_value, gen_jwt_token, jwt_token_to_data};

    #[test]
    fn test_check_phone_length_less() {
        let phone = "123123";
        let res = check_phone(phone);
        assert_eq!(res, false);
    }

    #[test]
    fn test_check_phone_length_large() {
        let phone = "2222222222222222222222222222222";
        let res = check_phone(phone);
        assert_eq!(res, false);
    }

    #[test]
    fn test_check_phone_reg_false() {
        let phone = "12717827650";
        let res = check_phone(phone);
        assert_eq!(res, false);
    }

    #[test]
    fn test_check_phone_reg_true() {
        let phone = "15717827650";
        let res = check_phone(phone);
        assert_eq!(res, true);
    }
    #[test]
    fn test_access_value() {
        // let role_p = [64, 1024];
        // let role: u32 = 64 + 1024;
        // role_p.map(|val| println!("res {}", val & role));

        let rs = gen_access_value(9999999);
        println!("res=== {rs}");
    }
    #[test]
    fn get_gen_jwt() {
        let login_data = RedisLoginData {
            auth: 123123123123,
            last_login_time: 12312312,
            name: "asdf".to_string(),
            id: 123,
        };
        let token_res = gen_jwt_token(login_data);
        println!("token_res {token_res}");
        let token = "eyJhbGciOiJIUzI1NiJ9.eyJhdXRoIjoxMjMxMjMxMjMxMjMsImxhc3RfbG9naW5fdGltZSI6MTIzMTIzMTIsIm5hbWUiOiJhc2RmIiwiaWQiOjEyM30.wqZusohUbF1MsrzttbL0Zf6jgvQXlSOwO7wwsvr06aE".to_string();
        assert_eq!(token_res, token)
    }

    #[test]
    fn test_jwt_token_to_data() {
        let token = "eyJhbGciOiJIUzI1NiJ9.eyJhdXRoIjoxMjMxMjMxMjMxMjMsImxhc3RfbG9naW5fdGltZSI6MTIzMTIzMTIsIm5hbWUiOiJhc2RmIiwiaWQiOjEyM30.wqZusohUbF1MsrzttbL0Zf6jgvQXlSOwO7wwsvr06aE".to_string();
        let login_data = RedisLoginData {
            auth: 123123123123,
            last_login_time: 12312312,
            name: "asdf".to_string(),
            id: 123,
        };
        let user_info = jwt_token_to_data(token);
        assert_eq!(login_data, user_info.unwrap());
    }
}