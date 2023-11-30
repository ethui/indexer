mod api;
mod config;
mod db;
mod provider;
mod sync;

use color_eyre::eyre::Result;
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};

use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    setup()?;

    let config = Config::read()?;
    let db = db::Db::connect(&config.db).await?;
    let sync = sync::Sync::start(db.clone(), &config).await?;
    let api = api::Api::start(db, config);

    // pin!(sync, db, api);
    let (sync, api) = futures::try_join!(sync, api)?;
    sync?;
    api?;

    Ok(())
}

fn setup() -> Result<()> {
    color_eyre::install()?;

    let filter = EnvFilter::from_default_env();

    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::NEW)
        .compact()
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}
