use super::error::Result;
use crate::db::{models::Register, Db};
use actix_web::{
    get, post,
    web::{self, Json},
    HttpResponse, Responder,
};

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok()
}

#[post("/register")]
async fn register(db: web::Data<Db>, Json(register): Json<Register>) -> Result<impl Responder> {
    db.register(register).await?;

    Ok(HttpResponse::Ok())
}
