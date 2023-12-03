mod backfill;
mod forward;
mod provider;
mod utils;

use std::{
    collections::{BTreeSet, HashSet},
    time::Duration,
};

use alloy_primitives::{Address, B256};
use async_trait::async_trait;
use color_eyre::eyre::Result;
use rand::{rngs::StdRng, SeedableRng};
use reth_primitives::Header;
use scalable_cuckoo_filter::{DefaultHasher, ScalableCuckooFilter, ScalableCuckooFilterBuilder};
use tokio::time::sleep;
use tracing::trace;

use crate::{
    config::Config,
    db::{
        models::{Chain, CreateTx},
        Db,
    },
};

pub use backfill::BackfillManager;
pub use forward::Forward;
pub use provider::{Provider, RethDBProvider};

/// Generic sync job state
pub struct Worker<T> {
    inner: T,
    provider: RethDBProvider,

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

impl<T> Worker<T> {
    async fn new(inner: T, db: Db, config: &Config, chain: Chain) -> Result<Self> {
        let provider = RethDBProvider::new(config, &chain)?;

        let mut cuckoo = ScalableCuckooFilterBuilder::new()
            .initial_capacity(1000)
            .rng(StdRng::from_entropy())
            .finish();

        config.sync.seed_addresses.iter().for_each(|addr| {
            cuckoo.insert(addr);
        });

        Ok(Self {
            inner,
            db,
            chain,
            addresses: config.sync.seed_addresses.clone(),
            cuckoo,
            provider,
            buffer: Vec::with_capacity(config.sync.buffer_size),
            buffer_capacity: config.sync.buffer_size,
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
            self.provider.reload()?;
            let latest = self.provider.last_block_number().unwrap();

            if latest >= block {
                trace!("new block(s) found. from: {}, latest: {}", block, latest);
                return Ok(());
            }

            sleep(Duration::from_secs(2)).await;
        }
    }

    async fn process_block(&mut self, header: &Header) -> Result<()> {
        for tx_id in self.provider.block_tx_id_ranges(header.number)? {
            let tx = match self.provider.tx_by_id(tx_id)? {
                Some(tx) => tx,
                None => continue,
            };

            let receipt = match self.provider.receipt_by_id(tx_id)? {
                Some(receipt) => receipt,
                None => continue,
            };

            let mut addresses: HashSet<_> = receipt
                .logs
                .into_iter()
                .flat_map(|log| log.topics.into_iter().filter_map(utils::topic_as_address))
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
