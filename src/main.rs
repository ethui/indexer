mod api;
mod config;
mod db;
mod provider;
mod sync;

use color_eyre::eyre::Result;
use tokio::sync::mpsc;
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};

use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    setup()?;

    let (account_tx, account_rx) = mpsc::unbounded_channel();
    let (job_tx, job_rx) = mpsc::unbounded_channel();
    let config = Config::read()?;
    let db = db::Db::connect(&config, account_tx, job_tx).await?;

    let sync = sync::MainSync::start(db.clone(), &config, account_rx).await?;
    let backfill = sync::BackfillSync::start(db.clone(), &config).await?;
    let api = api::Api::start(db, config);

    // pin!(sync, db, api);
    let (sync, backfill, api) = futures::try_join!(sync, backfill, api)?;
    sync?;
    backfill?;
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
