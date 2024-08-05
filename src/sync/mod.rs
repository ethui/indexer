mod backfill;
mod forward;
mod provider;
mod utils;

use std::{
    collections::{BTreeSet, HashSet},
    sync::Arc,
    time::Duration,
};

use alloy_primitives::{Address, B256};
use async_trait::async_trait;
pub use backfill::{BackfillManager, StopStrategy};
use color_eyre::eyre::{eyre, Result};
pub use forward::Forward;
pub use provider::RethProviderFactory;
use rand::{rngs::StdRng, SeedableRng};
use reth_primitives::Header;
use reth_provider::{BlockNumReader, BlockReader, ReceiptProvider, TransactionsProvider};
use scalable_cuckoo_filter::{DefaultHasher, ScalableCuckooFilter, ScalableCuckooFilterBuilder};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::trace;

use crate::{
    config::Config,
    db::{
        models::{Chain, CreateTx},
        Db,
    },
};

/// Generic sync job state
#[derive(Debug)]
pub struct Worker<T: std::fmt::Debug> {
    inner: T,

    provider_factory: Arc<RethProviderFactory>,

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

    /// How many times to check buffer capacity before forcing a flush
    max_buffer_tries: usize,

    /// How many tries since last flush
    current_buffer_tries: usize,

    /// Cancellation token for graceful shutdown
    cancellation_token: CancellationToken,
}

/// A match between an address and a transaction
#[derive(Debug)]
pub struct Match {
    pub address: Address,
    pub block_number: u64,
    pub hash: B256,
}

#[async_trait]
pub trait SyncJob {
    async fn run(mut self) -> Result<()>;
}

impl<T: std::fmt::Debug> Worker<T> {
    async fn new(
        inner: T,
        db: Db,
        config: &Config,
        chain: Chain,
        provider_factory: Arc<RethProviderFactory>,
        cancellation_token: CancellationToken,
    ) -> Result<Self> {
        let addresses: BTreeSet<_> = db.get_addresses().await?.into_iter().map(|a| a.0).collect();
        let mut cuckoo = ScalableCuckooFilterBuilder::new()
            .initial_capacity(addresses.len())
            .rng(StdRng::from_entropy())
            .finish();

        addresses.iter().for_each(|addr| {
            cuckoo.insert(addr);
        });

        Ok(Self {
            inner,
            provider_factory,
            db,
            chain,
            addresses,
            cuckoo,
            buffer: Vec::with_capacity(config.sync.buffer_size),
            buffer_capacity: config.sync.buffer_size,
            max_buffer_tries: config.sync.buffer_tries,
            current_buffer_tries: 0,
            cancellation_token,
        })
    }

    pub fn drain_buffer(&mut self) -> Vec<CreateTx> {
        self.buffer
            .drain(..)
            .map(|m| CreateTx {
                address: m.address.into(),
                chain_id: self.chain.chain_id,
                hash: m.hash.into(),
                block_number: m.block_number as i32,
            })
            .collect()
    }

    async fn wait_new_block(&mut self, block: u64) -> Result<()> {
        trace!(event = "wait", block);
        loop {
            let provider = self.provider_factory.get()?;

            let latest = provider.last_block_number().unwrap();

            if latest >= block {
                trace!("new block(s) found. from: {}, latest: {}", block, latest);
                return Ok(());
            }

            drop(provider);

            sleep(Duration::from_secs(2)).await;
        }
    }

    async fn process_block(&mut self, header: &Header) -> Result<()> {
        let provider = self.provider_factory.get()?;
        let indices = match provider.block_body_indices(header.number)? {
            Some(indices) => indices,
            None => return Err(eyre!("err")),
        };

        for tx_id in indices.first_tx_num..indices.first_tx_num + indices.tx_count {
            let tx = match provider.transaction_by_id_no_hash(tx_id)? {
                Some(tx) => tx,
                None => continue,
            };

            let receipt = match provider.receipt(tx_id)? {
                Some(receipt) => receipt,
                None => continue,
            };

            let mut addresses: HashSet<_> = receipt
                .logs
                .into_iter()
                .flat_map(|log| {
                    log.topics()
                        .iter()
                        .filter_map(utils::topic_as_address)
                        .collect::<Vec<_>>()
                })
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
