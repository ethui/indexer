mod api;
mod config;
mod db;
mod rearrange;
mod sync;

use std::sync::Arc;

use color_eyre::eyre::Result;
use tokio::sync::RwLock;
use tokio::{signal, sync::mpsc};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::info;
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};

use config::Config;

use crate::sync::{RethProvider, StopStrategy};

use self::db::Db;
use self::sync::{BackfillManager, Forward, SyncJob};

#[tokio::main]
async fn main() -> Result<()> {
    setup()?;

    let config = Config::read()?;

    // set up a few random things
    let (account_tx, account_rx) = mpsc::unbounded_channel();
    let (job_tx, job_rx) = mpsc::unbounded_channel();
    let db = Db::connect(&config, account_tx, job_tx).await?;
    let chain = db.setup_chain(&config.chain).await?;
    let provider_factory = Arc::new(RwLock::new(RethProvider::new(&config, &chain)?));
    let token = CancellationToken::new();

    // setup each task
    let sync = Forward::new(
        db.clone(),
        &config,
        chain,
        provider_factory.clone(),
        account_rx,
        token.clone(),
    )
    .await?;
    let backfill = BackfillManager::new(
        db.clone(),
        &config,
        provider_factory.clone(),
        job_rx,
        StopStrategy::Token(token.clone()),
    );
    let api = config.http.map(|c| api::start(db.clone(), c));

    // spawn and tasks and track them
    let tracker = TaskTracker::new();
    tracker.spawn(sync.run());
    tracker.spawn(backfill.run());
    api.map(|t| tracker.spawn(t));

    // termination handling
    signal::ctrl_c().await?;
    token.cancel();
    tracker.close();
    tracker.wait().await;

    info!("graceful shutdown achieved. Closing");

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
