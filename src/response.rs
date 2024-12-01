use actix_web::{
    body::BoxBody,
    error,
    http::{header::ContentType, StatusCode},
    HttpResponse, Responder,
};
use derive_more::derive::{Display, Error};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ResponseBody<T> {
    pub code: i16,
    pub msg: String,
    pub data: T,
}

impl<T> ResponseBody<T> {
    pub fn default(data: Option<T>) -> ResponseBody<Option<T>> {
        ResponseBody {
            code: 0,
            msg: "".to_string(),
            data,
        }
    }
}
impl ResponseBody<String> {
    pub fn error(msg: &str) -> ResponseBody<Option<String>> {
        ResponseBody {
            code: 500,
            msg: msg.to_string(),
            data: None,
        }
    }

    pub fn success(msg: &str) -> ResponseBody<Option<String>> {
        ResponseBody {
            code: 0,
            msg: msg.to_string(),
            data: None,
        }
    }
}
impl<T: Serialize> Responder for ResponseBody<T> {
    type Body = BoxBody;

    fn respond_to(self, _req: &actix_web::HttpRequest) -> actix_web::HttpResponse<Self::Body> {
        HttpResponse::Ok().force_close().json(&self)
    }
}

#[derive(Debug, Display, Error)]
pub enum MyError {
    #[display("internal error")]
    InternalError = 0,

    #[display("用户不存在")]
    UserNotExist,

    #[display("用户不正确")]
    UserIsWrong,

    #[display("密码错误")]
    PassWordError,
}

impl error::ResponseError for MyError {
    fn error_response(&self) -> HttpResponse {
        let rsp_data = ResponseBody::error(&self.to_string());
        HttpResponse::build(self.status_code())
            .insert_header(ContentType::json())
            .json(rsp_data)
    }
}
