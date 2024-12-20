use crate::{entity::role_entity::RoleEntity, util::structs::CreateByData, RB};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_actix_web::service_config::ServiceConfig;

mod role_access_service;
mod role_service;

pub fn configure() -> impl FnOnce(&mut ServiceConfig) {
    |config: &mut ServiceConfig| {
        config.service(role_service::create_role);
        config.service(role_service::get_role_list);
        config.service(role_service::update_role_by_id);
        config.service(role_service::bind_access);
        config.service(role_service::get_role_option);

        config.service(role_service::get_role_binds);
        config.service(role_service::delete_role_by_id);
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateRoleData {
    pub name: String,
    pub create_by: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct RoleUpdateData {
    pub id: i32,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct BindAccessData {
    pub role_id: i32,
    pub access_ids: Vec<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct AccessData {
    pub access_id: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct RoleListQueryData {
    pub create_by: Option<i32>,
    pub name: Option<String>,
    pub page_no: i32,
    pub take: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoleListListData {
    pub id: i32,
    pub create_time: String,
    pub update_time: String,
    pub name: String,
    pub create_by: Option<CreateByData>, // 创建的用户id
    pub status: i8,
}

pub async fn check_role_by_id(id: i32) -> Option<RoleEntity> {
    let ex_db = RB.acquire().await.expect("get db ex error");
    let db_role = RoleEntity::select_by_id(&ex_db, id.into())
        .await
        .expect("角色查询失败");

    db_role
}
