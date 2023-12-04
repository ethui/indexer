use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use color_eyre::eyre::Result;
use tokio::select;
use tokio::{
    sync::{mpsc::UnboundedReceiver, RwLock, Semaphore},
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument};

use crate::{
    config::Config,
    db::{models::BackfillJobWithId, Db},
};

use super::provider::Provider;
use super::{SyncJob, Worker};

#[derive(Debug)]
pub enum StopStrategy {
    Token(CancellationToken),
    OnFinish,
}

impl StopStrategy {
    fn is_on_finish(&self) -> bool {
        match self {
            StopStrategy::Token(_) => true,
            _ => false,
        }
    }
}

/// Backfill job
/// Walks the blockchain backwards, within a fixed range
/// Processes a list of addresses determined by the rearrangment logic defined in
/// `crate::db::rearrange_backfill`
pub struct BackfillManager {
    db: Db,
    concurrency: usize,
    jobs_rcv: UnboundedReceiver<()>,
    config: Arc<RwLock<Config>>,
    stop: StopStrategy,
}

impl BackfillManager {
    pub fn new(
        db: Db,
        config: &Config,
        jobs_rcv: UnboundedReceiver<()>,
        stop: StopStrategy,
    ) -> Self {
        Self {
            db,
            jobs_rcv,
            config: Arc::new(RwLock::new(config.clone())),
            concurrency: config.sync.backfill_concurrency,
            stop,
        }
    }

    #[instrument(name = "backfill", skip(self), fields(concurrency = self.concurrency))]
    pub async fn run(mut self) -> Result<()> {
        loop {
            let semaphore = Arc::new(Semaphore::new(self.concurrency));
            let inner_cancel = CancellationToken::new();
            dbg!(self.concurrency);

            self.db.reorg_backfill_jobs().await?;
            let jobs = self.db.get_backfill_jobs().await?;
            dbg!(jobs.len());

            if self.stop.is_on_finish() && jobs.is_empty() {
                break;
            }

            let workers = jobs
                .into_iter()
                .map(|job| {
                    let db = self.db.clone();
                    let semaphore = semaphore.clone();
                    let config = self.config.clone();
                    let token = inner_cancel.clone();
                    tokio::spawn(async move {
                        let _permit = semaphore.acquire().await.unwrap();
                        if token.is_cancelled() {
                            return Ok(());
                        }
                        let worker = Backfill::new_worker(db, config, job, token).await.unwrap();
                        worker.run().await
                    })
                })
                .collect::<Vec<_>>();

            // wait for a new job, or a preset delay, whichever comes first
            match &self.stop {
                // stop when cancellation token signals
                // wait for new jobs too, which should be a sign to reorg
                // request each job to stop
                StopStrategy::Token(token) => {
                    let timeout = sleep(Duration::from_secs(10 * 60));
                    select! {
                        _ = token.cancelled() => {}
                        _ = timeout => {}
                        Some(_) = self.jobs_rcv.recv() => {}
                    }
                    inner_cancel.cancel();
                    for worker in workers {
                        worker.await.unwrap().unwrap();
                    }
                    info!("closing backfill manager");
                }

                // if we stop on finish, no need to do anything here
                StopStrategy::OnFinish => {
                    for worker in workers {
                        worker.await.unwrap().unwrap();
                    }
                    break;
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct Backfill {
    job_id: i32,
    high: u64,
    low: u64,
}

#[async_trait]
impl SyncJob for Worker<Backfill> {
    #[instrument(skip(self), fields(chain_id = self.chain.chain_id))]
    async fn run(mut self) -> Result<()> {
        for block in (self.inner.low..self.inner.high).rev() {
            // start by checking shutdown signal
            if self.cancellation_token.is_cancelled() {
                break;
            }

            let header = self.provider.block_header(block)?.unwrap();
            self.process_block(&header).await?;
            self.maybe_flush(block).await?;
        }

        self.flush(self.inner.low).await?;

        info!("closing backfill worker");
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
        cancellation_token: CancellationToken,
    ) -> Result<Worker<Self>> {
        let config = config.read().await;
        let chain = db.setup_chain(&config.chain).await?;

        let s = Self {
            job_id: job.id,
            high: job.high as u64,
            low: job.low as u64,
        };

        Worker::new(s, db, &config, chain, cancellation_token).await
    }
}
