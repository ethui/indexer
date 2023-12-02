use async_trait::async_trait;
use color_eyre::eyre::Result;
use reth_provider::HeaderProvider;
use tokio::task::JoinHandle;
use tracing::instrument;

use crate::config::Config;
use crate::db::Db;

use super::{SyncInner, SyncJob};

/// Backfill job
/// Walks the blockchain backwards, within a fixed range
/// Processes a list of addresses determined by the rearrangment logic defined in
/// `crate::db::rearrange_backfill`
pub struct BackfillSync {}

impl BackfillSync {
    pub async fn start(db: Db, config: &Config) -> Result<JoinHandle<Result<()>>> {
        let sync = Self {
            // inner: SyncInner::new(db, config).await?,
        };

        Ok(tokio::spawn(async move { sync.run().await }))
    }
}

pub struct Worker {
    inner: SyncInner,
    from: u64,
    to: u64,
}

#[async_trait]
impl SyncJob for Worker {
    #[instrument(skip(self), fields(chain_id = self.inner.chain.chain_id))]
    async fn run(mut self) -> Result<()> {
        for block in (self.from..=self.to).rev() {
            self.inner.next_block = block;
            let header = self.inner.provider.header_by_number(block)?.unwrap();
            self.inner.process_block(&header).await?;
            self.inner.maybe_flush().await?;
        }

        Ok(())
    }
}
