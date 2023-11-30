mod error;
mod routes;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use color_eyre::eyre::Result;
use tokio::task::JoinHandle;
use tracing_actix_web::TracingLogger;

use crate::config::Config;
use crate::db::Db;

pub struct Api;

impl Api {
    pub fn start(config: Config) -> JoinHandle<Result<()>> {
        tokio::spawn(async move { run(config).await })
    }
}

#[tracing::instrument(name = "api", skip(config), fields(port = config.http.port))]
pub async fn run(config: Config) -> Result<()> {
    let db = Db::connect(&config.db).await?;

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .wrap(TracingLogger::default())
            .service(routes::health)
            .service(routes::register)
            .app_data(web::Data::new(db.clone()))
    })
    .disable_signals()
    .bind(("0.0.0.0", config.http.port))
    .unwrap()
    .run()
    .await
    .unwrap();

    Ok(())
}
