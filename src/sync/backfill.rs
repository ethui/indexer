use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use color_eyre::eyre::Result;
use reth_provider::HeaderProvider;
use tokio::select;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::{broadcast, RwLock, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::instrument;

use crate::config::Config;
use crate::db::models::BackfillJobWithId;
use crate::db::Db;

use super::{SyncInner, SyncJob};

/// Backfill job
/// Walks the blockchain backwards, within a fixed range
/// Processes a list of addresses determined by the rearrangment logic defined in
/// `crate::db::rearrange_backfill`
pub struct BackfillSync {
    db: Db,
    concurrency: usize,
    jobs_rcv: UnboundedReceiver<()>,
    config: Arc<RwLock<Config>>,
}

impl BackfillSync {
    pub async fn start(
        db: Db,
        config: &Config,
        jobs_rcv: UnboundedReceiver<()>,
    ) -> Result<JoinHandle<Result<()>>> {
        let sync = Self {
            db,
            jobs_rcv,
            config: Arc::new(RwLock::new(config.clone())),
            concurrency: config.sync.backfill_concurrency,
        };

        Ok(tokio::spawn(async move { sync.run().await }))
    }

    #[instrument(skip(self))]
    async fn run(mut self) -> Result<()> {
        loop {
            let semaphore = Arc::new(Semaphore::new(self.concurrency));
            let (shutdown, _) = broadcast::channel(1);

            self.db.rearrange_backfill_jobs().await?;

            let workers = self
                .db
                .get_backfill_jobs()
                .await?
                .into_iter()
                .map(|job| {
                    let db = self.db.clone();
                    let semaphore = semaphore.clone();
                    let config = self.config.clone();
                    let mut shutdown = shutdown.subscribe();
                    tokio::spawn(async move {
                        let _permit = semaphore.acquire().await.unwrap();
                        if shutdown.try_recv().is_ok() {
                            return Ok(());
                        }
                        let worker = Worker::new(db, config, job, shutdown).await.unwrap();
                        worker.run().await
                    })
                })
                .collect::<Vec<_>>();

            // wait for a new job, or a preset delay, whichever comes first
            let timeout = sleep(Duration::from_secs(10 * 60));
            select! {
                _ = timeout => {}
                _ = self.jobs_rcv.recv() => {}
            }

            // shutdown, time to re-org and reprioritize
            shutdown.send(())?;
            for worker in workers {
                worker.await.unwrap().unwrap();
            }
        }
    }
}

pub struct Worker {
    inner: SyncInner,
    from: u64,
    to: u64,
    shutdown: broadcast::Receiver<()>,
}

#[async_trait]
impl SyncJob for Worker {
    #[instrument(skip(self), fields(chain_id = self.inner.chain.chain_id))]
    async fn run(mut self) -> Result<()> {
        for block in (self.from..=self.to).rev() {
            // start by checking shutdown signal
            if self.shutdown.try_recv().is_ok() {
                break;
            }

            self.inner.next_block = block;
            let header = self.inner.provider.header_by_number(block)?.unwrap();
            self.inner.process_block(&header).await?;
            self.inner.maybe_flush().await?;
        }

        // TODO: flush needs to properly update the job
        // this needs to be part of BackfillJob, not just Inner
        self.inner.flush().await?;

        Ok(())
    }
}

impl Worker {
    async fn new(
        db: Db,
        config: Arc<RwLock<Config>>,
        job: BackfillJobWithId,
        shutdown: broadcast::Receiver<()>,
    ) -> Result<Self> {
        let config = config.read().await;
        Ok(Self {
            from: job.to_block as u64,
            to: job.from_block as u64,
            shutdown,
            inner: SyncInner::new(db, &config).await?,
        })
    }
}
