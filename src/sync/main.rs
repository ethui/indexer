use alloy_primitives::Address;
use async_trait::async_trait;
use color_eyre::eyre::Result;
use reth_provider::HeaderProvider;
use tokio::{sync::mpsc::UnboundedReceiver, task::JoinHandle};
use tracing::instrument;

use crate::{config::Config, db::Db};

use super::{SyncInner, SyncJob};

/// Main sync job
/// Walks the blockchain forward, from a pre-configured starting block.
/// Once it reaches the tip, waits continuously for new blocks to process
///
/// Receives events for newly registered addresses, at which point they are added to the search set
/// and a backfill job is scheduled
pub struct MainSync {
    /// Sync job state
    inner: SyncInner,

    /// Receiver for account registration events
    accounts_rcv: UnboundedReceiver<Address>,
}

#[async_trait]
impl SyncJob for MainSync {
    #[instrument(skip(self), fields(chain_id = self.inner.chain.chain_id))]
    async fn run(mut self) -> Result<()> {
        loop {
            self.process_new_accounts().await?;

            match self
                .inner
                .provider
                .header_by_number(self.inner.next_block)?
            {
                // got a block. process it, only flush if needed
                Some(header) => {
                    self.inner.process_block(&header).await?;
                    self.inner.maybe_flush().await?;
                    self.inner.next_block += 1;
                }

                // no block found. take the wait chance to flush, and wait for new block
                None => {
                    self.inner.flush().await?;
                    self.inner.wait_new_block(self.inner.next_block).await?;
                }
            }
        }
    }
}

impl MainSync {
    pub async fn start(
        db: Db,
        config: &Config,
        accounts_rcv: UnboundedReceiver<Address>,
    ) -> Result<JoinHandle<Result<()>>> {
        let sync = Self {
            inner: SyncInner::new(db, config).await?,
            accounts_rcv,
        };

        Ok(tokio::spawn(async move { sync.run().await }))
    }

    pub async fn process_new_accounts(&mut self) -> Result<()> {
        while let Ok(address) = self.accounts_rcv.try_recv() {
            self.inner.addresses.insert(address);
            self.inner.cuckoo.insert(&address);
            self.setup_backfill(address).await?;
        }
        Ok(())
    }

    /// Create a new job for backfilling history for a new account
    /// before the current sync point
    async fn setup_backfill(&mut self, address: Address) -> Result<()> {
        self.inner
            .db
            .create_backfill_job(
                address.into(),
                self.inner.chain.chain_id,
                self.inner.chain.start_block,
                (self.inner.next_block - 1) as i32,
            )
            .await?;
        Ok(())
    }
}
