use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use color_eyre::eyre::Result;
use tokio::select;
use tokio::{
    sync::{broadcast, mpsc::UnboundedReceiver, RwLock, Semaphore},
    task::JoinHandle,
    time::sleep,
};
use tracing::instrument;

use crate::{
    config::Config,
    db::{models::BackfillJobWithId, Db},
};

use super::provider::Provider;
use super::{SyncJob, Worker};

/// Backfill job
/// Walks the blockchain backwards, within a fixed range
/// Processes a list of addresses determined by the rearrangment logic defined in
/// `crate::db::rearrange_backfill`
pub struct BackfillManager {
    db: Db,
    concurrency: usize,
    jobs_rcv: UnboundedReceiver<()>,
    config: Arc<RwLock<Config>>,
}

impl BackfillManager {
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
                        let worker = Backfill::new_worker(db, config, job, shutdown)
                            .await
                            .unwrap();
                        worker.run().await
                    })
                })
                .collect::<Vec<_>>();

            // wait for a new job, or a preset delay, whichever comes first
            let timeout = sleep(Duration::from_secs(10 * 60));
            select! {
                _ = timeout => {dbg!("timeout");}
                Some(_) = self.jobs_rcv.recv() => {dbg!("rcv");}
            }

            // shutdown, time to re-org and reprioritize
            dbg!("sending");
            shutdown.send(())?;
            for worker in workers {
                worker.await.unwrap().unwrap();
            }
        }
    }
}

pub struct Backfill {
    job_id: i32,
    high: u64,
    low: u64,
    shutdown: broadcast::Receiver<()>,
}
#[async_trait]
impl SyncJob for Worker<Backfill> {
    #[instrument(skip(self), fields(chain_id = self.chain.chain_id))]
    async fn run(mut self) -> Result<()> {
        for block in (self.inner.low..self.inner.high).rev() {
            // start by checking shutdown signal
            if self.inner.shutdown.try_recv().is_ok() {
                break;
            }

            let header = self.provider.block_header(block)?.unwrap();
            self.process_block(&header).await?;
            self.maybe_flush(block).await?;
        }

        // TODO: flush needs to properly update the job
        // this needs to be part of BackfillJob, not just Inner
        self.flush(self.inner.low).await?;

        Ok(())
    }
}

impl Worker<Backfill> {
    /// if the buffer is sufficiently large, flush it to the database
    /// and update chain tip
    pub async fn maybe_flush(&mut self, last_block: u64) -> Result<()> {
        if self.buffer.len() >= self.buffer_capacity {
            self.flush(last_block).await?;
        }

        Ok(())
    }

    // empties the buffer and updates chain tip
    pub async fn flush(&mut self, last_block: u64) -> Result<()> {
        let txs = self.drain_buffer();

        self.db.create_txs(txs).await?;
        self.db.update_job(self.inner.job_id, last_block).await?;

        Ok(())
    }
}

impl Backfill {
    async fn new_worker(
        db: Db,
        config: Arc<RwLock<Config>>,
        job: BackfillJobWithId,
        shutdown: broadcast::Receiver<()>,
    ) -> Result<Worker<Self>> {
        let config = config.read().await;
        let chain = db.setup_chain(&config.chain).await?;

        let s = Self {
            job_id: job.id,
            high: job.high as u64,
            low: job.low as u64,
            shutdown,
        };

        Worker::new(s, db, &config, chain).await
    }
}
