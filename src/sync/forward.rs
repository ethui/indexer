use alloy_primitives::Address;
use async_trait::async_trait;
use color_eyre::eyre::Result;
use tokio::{sync::mpsc::UnboundedReceiver, task::JoinHandle};
use tracing::instrument;

use crate::{config::Config, db::Db};

use super::provider::Provider;
use super::{SyncJob, Worker};

/// Main sync job
/// Walks the blockchain forward, from a pre-configured starting block.
/// Once it reaches the tip, waits continuously for new blocks to process
///
/// Receives events for newly registered addresses, at which point they are added to the search set
/// and a backfill job is scheduled
pub struct Forward {
    /// Receiver for account registration events
    accounts_rcv: UnboundedReceiver<Address>,
    next_block: u64,
}

#[async_trait]
impl SyncJob for Worker<Forward> {
    #[instrument(skip(self), fields(chain_id = self.chain.chain_id))]
    async fn run(mut self) -> Result<()> {
        self.inner.next_block = (self.chain.last_known_block as u64) + 1;

        loop {
            self.process_new_accounts().await?;

            match self.provider.block_header(self.inner.next_block)? {
                // got a block. process it, only flush if needed
                Some(header) => {
                    self.process_block(&header).await?;
                    self.maybe_flush().await?;
                    self.inner.next_block += 1;
                }

                // no block found. take the wait chance to flush, and wait for new block
                None => {
                    self.flush().await?;
                    self.wait_new_block(self.inner.next_block).await?;
                }
            }
        }
    }
}

impl Worker<Forward> {
    pub async fn process_new_accounts(&mut self) -> Result<()> {
        while let Ok(address) = self.inner.accounts_rcv.try_recv() {
            self.addresses.insert(address);
            self.cuckoo.insert(&address);
            self.setup_backfill(address).await?;
        }
        Ok(())
    }

    /// Create a new job for backfilling history for a new account
    /// before the current sync point
    async fn setup_backfill(&mut self, address: Address) -> Result<()> {
        self.db
            .create_backfill_job(
                address.into(),
                self.chain.start_block,
                self.inner.next_block as i32,
            )
            .await?;
        Ok(())
    }

    /// if the buffer is sufficiently large, flush it to the database
    /// and update chain tip
    pub async fn maybe_flush(&mut self) -> Result<()> {
        if self.buffer.len() >= self.buffer_capacity {
            self.flush().await?;
        }

        Ok(())
    }

    // empties the buffer and updates chain tip
    pub async fn flush(&mut self) -> Result<()> {
        let txs = self.drain_buffer();

        self.db.create_txs(txs).await?;
        self.db
            .update_chain(self.chain.chain_id as u64, self.inner.next_block)
            .await?;

        Ok(())
    }
}

impl Forward {
    pub async fn start(
        db: Db,
        config: &Config,
        accounts_rcv: UnboundedReceiver<Address>,
    ) -> Result<JoinHandle<Result<()>>> {
        let chain = db.setup_chain(&config.chain).await?;

        let sync = Worker::new(
            Forward {
                accounts_rcv,
                next_block: (chain.last_known_block as u64) + 1,
            },
            db,
            config,
            chain,
        )
        .await?;

        Ok(tokio::spawn(async move { sync.run().await }))
    }
}
