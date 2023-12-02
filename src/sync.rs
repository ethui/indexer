use std::{
    collections::{BTreeSet, HashSet},
    time::Duration,
};

use alloy_primitives::{Address, FixedBytes, B256};
use async_trait::async_trait;
use color_eyre::eyre::Result;
use rand::{rngs::StdRng, SeedableRng};
use reth_db::{
    mdbx::{tx::Tx, RO},
    DatabaseEnv,
};
use reth_primitives::Header;
use reth_provider::{
    BlockNumReader, BlockReader, DatabaseProvider, HeaderProvider, ProviderFactory,
    ReceiptProvider, TransactionsProvider,
};
use scalable_cuckoo_filter::{DefaultHasher, ScalableCuckooFilter, ScalableCuckooFilterBuilder};
use tokio::{sync::mpsc::UnboundedReceiver, task::JoinHandle, time::sleep};
use tracing::{instrument, trace};

use crate::{
    config::Config,
    db::{
        models::{Chain, CreateTx},
        Db,
    },
    events::Event,
    provider::provider_factory,
};

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
    rcv: UnboundedReceiver<Event>,
}

/// Backfill job
/// Walks the blockchain backwards, within a fixed range
/// Processes a list of addresses determined by the rearrangment logic defined in
/// `crate::db::rearrange_backfill`
pub struct BackfillSync {
    inner: SyncInner,
    from: u64,
    to: u64,
}

/// Generic sync job state
struct SyncInner {
    /// DB handle
    db: Db,

    /// Chain configuration
    chain: Chain,

    /// Set of addresses to search for
    addresses: BTreeSet<Address>,

    /// Cuckoo filter for fast address inclusion check
    cuckoo: ScalableCuckooFilter<Address, DefaultHasher, StdRng>,

    /// Buffer holding matches to be written to the database
    buffer: Vec<Match>,

    /// Desired buffer capacity, and threshold at which to flush it
    buffer_capacity: usize,

    /// Current block number being processed or waited for
    next_block: u64,

    /// Reth Provider factory
    factory: ProviderFactory<DatabaseEnv>,

    /// Current Reth DB provider
    provider: DatabaseProvider<Tx<RO>>,
}

/// A match between an address and a transaction
#[derive(Debug)]
pub struct Match {
    pub address: Address,
    pub block_number: u64,
    pub hash: B256,
}

#[async_trait]
trait SyncJob {
    async fn run(mut self) -> Result<()>;
}

#[async_trait]
impl SyncJob for MainSync {
    #[instrument(skip(self), fields(chain_id = self.inner.chain.chain_id))]
    async fn run(mut self) -> Result<()> {
        loop {
            self.process_events().await?;

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

#[async_trait]
impl SyncJob for BackfillSync {
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

impl MainSync {
    pub async fn start(
        db: Db,
        config: &Config,
        rcv: UnboundedReceiver<Event>,
    ) -> Result<JoinHandle<Result<()>>> {
        let sync = Self {
            inner: SyncInner::new(db, config).await?,
            rcv,
        };

        Ok(tokio::spawn(async move { sync.run().await }))
    }

    pub async fn process_events(&mut self) -> Result<()> {
        while let Ok(event) = self.rcv.try_recv() {
            match event {
                Event::AccountRegistered { address } => {
                    self.inner.addresses.insert(address);
                    self.inner.cuckoo.insert(&address);
                    self.setup_backfill(address).await?;
                }
            }
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

impl SyncInner {
    async fn new(db: Db, config: &Config) -> Result<Self> {
        let chain = db.setup_chain(&config.chain).await?;
        let factory = provider_factory(chain.chain_id as u64, &config.reth)?;
        let provider: reth_provider::DatabaseProvider<Tx<RO>> = factory.provider()?;

        let mut cuckoo = ScalableCuckooFilterBuilder::new()
            .initial_capacity(1000)
            .rng(StdRng::from_entropy())
            .finish();

        config.sync.seed_addresses.iter().for_each(|addr| {
            cuckoo.insert(addr);
        });

        Ok(Self {
            db,
            next_block: chain.last_known_block as u64 + 1,
            chain,
            addresses: config.sync.seed_addresses.clone(),
            cuckoo,
            factory,
            provider,
            buffer: Vec::with_capacity(config.sync.buffer_size),
            buffer_capacity: config.sync.buffer_size,
        })
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
        let txs: Vec<_> = self
            .buffer
            .drain(..)
            .map(|m| CreateTx {
                address: m.address.into(),
                chain_id: self.chain.chain_id,
                hash: m.hash.into(),
                block_number: m.block_number as i32,
            })
            .collect();

        self.db.create_txs(txs).await?;
        self.db
            .update_chain(self.chain.chain_id as u64, self.next_block)
            .await?;

        Ok(())
    }

    async fn wait_new_block(&mut self, block: u64) -> Result<()> {
        trace!(event = "wait", block);
        loop {
            let provider = self.factory.provider()?;
            let latest = provider.last_block_number().unwrap();

            if latest >= block {
                trace!("new block(s) found. from: {}, latest: {}", block, latest);
                self.provider = provider;
                return Ok(());
            }

            sleep(Duration::from_secs(2)).await;
        }
    }

    async fn process_block(&mut self, header: &Header) -> Result<()> {
        let indices = match self.provider.block_body_indices(header.number)? {
            Some(indices) => indices,
            None => return Ok(()),
        };

        for tx_id in indices.first_tx_num..indices.first_tx_num + indices.tx_count {
            let tx = match self.provider.transaction_by_id_no_hash(tx_id)? {
                Some(tx) => tx,
                None => continue,
            };

            let receipt = match self.provider.receipt(tx_id)? {
                Some(receipt) => receipt,
                None => continue,
            };

            let mut addresses: HashSet<_> = receipt
                .logs
                .into_iter()
                .flat_map(|log| log.topics.into_iter().filter_map(topic_as_address))
                .collect();

            tx.recover_signer().map(|a| addresses.insert(a));
            tx.to().map(|a| addresses.insert(a));

            addresses
                .into_iter()
                .filter(|addr| self.cuckoo.contains(addr))
                .filter(|addr| self.addresses.contains(addr))
                .for_each(|address| {
                    self.buffer.push(Match {
                        address,
                        block_number: header.number,
                        hash: tx.hash(),
                    })
                });
        }

        Ok(())
    }
}

fn topic_as_address(topic: FixedBytes<32>) -> Option<Address> {
    let padding_slice = &topic.as_slice()[0..12];
    let padding: FixedBytes<12> = FixedBytes::from_slice(padding_slice);

    if padding.is_zero() {
        Some(Address::from_slice(&topic[12..]))
    } else {
        None
    }
}
