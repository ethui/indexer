mod error;
mod routes;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use tokio::task::JoinHandle;
use tracing::instrument;
use tracing_actix_web::TracingLogger;

use crate::{config::HttpConfig, db::Db};

#[instrument(name = "api", skip(db, config), fields(port = config.port))]
pub fn start(db: Db, config: HttpConfig) -> JoinHandle<std::result::Result<(), std::io::Error>> {
    let server = HttpServer::new(move || {
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
    .bind(("0.0.0.0", config.port))
    .unwrap();

    tokio::spawn(server.run())
}
