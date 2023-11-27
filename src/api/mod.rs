mod routes;

use actix_cors::Cors;
use actix_web::{App, HttpServer};
use color_eyre::eyre::Result;
use tokio::task::JoinHandle;
use tracing_actix_web::TracingLogger;

use crate::config::HttpConfig;

pub struct Api;

impl Api {
    pub fn start(config: HttpConfig) -> JoinHandle<Result<()>> {
        tokio::spawn(async move { run(config).await })
    }
}

#[tracing::instrument(name = "api", skip(config),fields(port = config.port))]
pub async fn run(config: HttpConfig) -> Result<()> {
    // info!("Starting server on 0.0.0.0:{}", config.port);

    HttpServer::new(|| {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .wrap(TracingLogger::default())
            .service(routes::health)
    })
    .disable_signals()
    .bind(("0.0.0.0", config.port))
    .unwrap()
    .run()
    .await
    .unwrap();

    Ok(())
}
