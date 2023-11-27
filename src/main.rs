mod api;
mod config;
mod db;
mod provider;
mod sync;

use color_eyre::eyre::Result;
use config::Config;
use futures::future;
use tokio::pin;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .with_span_events(FmtSpan::NEW)
        .compact()
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let config = Config::read()?;
    let sync = sync::Sync::start(&config)?;
    let api = api::server(&config.http);

    pin!(sync, api);
    future::select(sync, api).await;

    Ok(())
}
