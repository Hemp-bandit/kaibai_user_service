use super::LoginData;
use crate::{
    access::AccessValueData,
    response::{MyError, ResponseBody},
    role::AccessData,
    user::{check_user_by_user_id, RedisLoginData},
    util::{
        common::get_jwt_from_req,
        redis_actor::{DelData, ExistsData, GetRedisLogin, SetRedisLogin, UpdateLoginData},
    },
    RB, REDIS_ADDR, REDIS_KEY,
};
use actix_web::{get, post, web, HttpRequest, Responder};
use rbs::to_value;
use rs_service_util::{jwt::gen_jwt_token, redis::RedisCmd, time::get_current_timestamp};
use serde::{Deserialize, Serialize};

const LOGIN_EX_TIME: u64 = 60 * 60 * 24 * 10;

#[derive(Serialize, Deserialize)]
struct PasswordData {
    password: String,
    name: String,
    id: i32,
}

#[utoipa::path(
  tag = "auth",
  responses( (status = 200))
)]
#[get("/get_user_permission/{id}")]
async fn get_user_permission(id: web::Path<i32>) -> Result<impl Responder, MyError> {
    let id = id.into_inner();
    let check_res = check_user_by_user_id(id).await;
    if check_res.is_none() {
        return Err(MyError::UserNotExist);
    }
    let vals = get_user_access_val(id).await;

    let db_uer = check_res.unwrap();

    let key = format!("{}_{}", REDIS_KEY.to_string(), db_uer.name.clone());
    let rds = REDIS_ADDR.get().expect("msg");

    let mut user = rds
        .send(GetRedisLogin { key: key.clone() })
        .await
        .expect("get user info from redis error")
        .expect("get user info from redis error")
        .unwrap();

    user.auth = vals;

    rds.send(UpdateLoginData { key, data: user })
        .await
        .expect("msg")
        .expect("msg");

    Ok(ResponseBody::default(Some(vals)))
}

#[utoipa::path(
    tag = "auth",
    responses( (status = 200) )
)]
#[post("/login")]
async fn login(req_data: web::Json<LoginData>) -> Result<impl Responder, MyError> {
    let key = format!("{}_{}", REDIS_KEY.to_string(), req_data.name.clone());
    let rds = REDIS_ADDR.get().expect("msg");
    let redis_login: Result<bool, redis::RedisError> = rds
        .send(ExistsData {
            key: key.clone(),
            cmd: RedisCmd::Exists,
            data: None,
        })
        .await
        .expect("msg");
    log::info!("redis_login {redis_login:?}");
    let is_login = match redis_login {
        Err(err) => {
            let detail = err.detail().expect("msg");
            log::error!("{}", detail);
            return Ok(ResponseBody::error(detail));
        }
        Ok(res) => res,
    };

    if is_login {
        let user_info = rds
            .send(GetRedisLogin { key })
            .await
            .expect("get user info from redis error")
            .expect("get user info from redis error");
        match user_info {
            None => {
                return Err(MyError::UserNotExist);
            }
            Some(info) => {
                match check_user_pass_by_name(info.name.clone()).await {
                    None => {
                        return Err(MyError::UserNotExist);
                    }
                    Some(pass_data) => {
                        if !pass_data.password.eq(&req_data.password) {
                            return Err(MyError::PassWordError);
                        }
                    }
                }
                let jwt_token = gen_jwt_token(info.clone());
                return Ok(ResponseBody::default(Some(jwt_token)));
            }
        }
    }

    let db_user = check_user_pass_by_name(req_data.name.clone()).await;

    if db_user.is_none() {
        return Err(MyError::UserNotExist);
    }
    let db_user = db_user.unwrap();

    let is_eq = db_user.password.eq(&req_data.password);
    if !is_eq {
        return Err(MyError::PassWordError);
    }
    let auth: u64 = get_user_access_val(db_user.id).await;
    let redis_data = RedisLoginData {
        auth,
        last_login_time: get_current_timestamp(),
        name: req_data.name.clone(),
        id: db_user.id.clone(),
    };

    let _ = rds
        .send(SetRedisLogin {
            key: key.clone(),
            data: redis_data.clone(),
            ex_data: LOGIN_EX_TIME,
        })
        .await
        .expect("set user login error");
    let jwt_token = gen_jwt_token(redis_data);
    return Ok(ResponseBody::default(Some(jwt_token)));
}

#[utoipa::path(
    tag = "auth",
    responses( (status = 200) )
)]
#[post("/logout/{id}")]
async fn logout(id: web::Path<i32>, req: HttpRequest) -> Result<impl Responder, MyError> {
    let user_id = id.into_inner();
    let check_res = check_user_by_user_id(user_id).await;
    if check_res.is_none() {
        return Err(MyError::UserNotExist);
    }
    let jwt_user = get_jwt_from_req(req);
    if jwt_user.id != user_id {
        return Err(MyError::UserIsWrong);
    }
    delete_user_from_redis(jwt_user.name).await;

    Ok(ResponseBody::success("退出成功!"))
}

/// 根据用户id 获取所有权限值
pub async fn get_user_access_val(user_id: i32) -> u64 {
    let mut vals: u64 = 0;
    let ex = RB.acquire().await.expect("get ex error");
    let access_ids: Option<Vec<AccessData>> = ex
        .query_decode(
            "select role_access.access_id from role_access where role_id in (select user_role.role_id from user_role where user_id=?)",
            vec![to_value!(user_id)],
        )
        .await
        .expect("查询权限id错误");

    if let Some(access_id_vec) = access_ids {
        let ids: Vec<String> = access_id_vec
            .into_iter()
            .map(|val| val.access_id.to_string())
            .collect();
        let ids = ids.join(",");
        let access_values: Option<Vec<AccessValueData>> = ex
            .query_decode(
                &format!("select value from access where id in ({ids})"),
                vec![],
            )
            .await
            .expect("查询权限值错误");

        log::info!("access_values {access_values:?}");
        if let Some(access_values) = access_values {
            access_values.into_iter().for_each(|val| {
                vals += val.value;
            });
        }
    }
    vals
}

async fn check_user_pass_by_name(name: String) -> Option<PasswordData> {
    let ex = RB.acquire().await.expect("msg");

    let db_user: Option<PasswordData> = ex
        .query_decode(
            "select password, id, name from user where user.name=?",
            vec![to_value!(name)],
        )
        .await
        .expect("获取用户失败");
    db_user
}

async fn delete_user_from_redis(user_name: String) {
    let key = format!("{}_{}", REDIS_KEY.to_string(), user_name);
    let rds = REDIS_ADDR.get().expect("msg");
    let _ = rds.send(DelData { key }).await.expect("delete error");
}
