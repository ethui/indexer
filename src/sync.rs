use std::collections::HashSet;
use std::{collections::BTreeSet, time::Duration};

use alloy_primitives::{Address, FixedBytes, B256};
use color_eyre::eyre::Result;
use rand::rngs::StdRng;
use rand::SeedableRng;
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
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::{task::JoinHandle, time::sleep};
use tracing::{instrument, trace};

use crate::config::Config;
use crate::db::models::Chain;
use crate::db::{models::CreateTx, Db};
use crate::events::Event;
use crate::provider::provider_factory;

#[derive(Debug)]
pub struct Match {
    pub address: Address,
    pub block_number: u64,
    pub hash: B256,
}

struct MainSync {
    next_block: u64,
    rcv: UnboundedReceiver<Event>,
    sync: SyncData,
}

struct BackfillSync {
    from: u64,
    to: u64,
    sync: SyncData,
}

trait SyncType: Send + 'static {}
impl SyncType for MainSync {}
impl SyncType for BackfillSync {}

struct SyncData {
    db: Db,
    chain: Chain,
    addresses: BTreeSet<Address>,
    cuckoo: ScalableCuckooFilter<Address, DefaultHasher, StdRng>,
    factory: ProviderFactory<DatabaseEnv>,
    provider: DatabaseProvider<Tx<RO>>,
    buffer: Vec<Match>,
    buffer_capacity: usize,
    next_block: u64,
    rcv: UnboundedReceiver<Event>,
}

pub async fn start_main(
    db: Db,
    config: &Config,
    rcv: UnboundedReceiver<Event>,
) -> Result<JoinHandle<Result<()>>> {
    let sync = Sync::<MainSync>::new(db, config, rcv).await?;
    Ok(tokio::spawn(async move { sync.run().await }))
}

impl MainSync {
    async fn new(db: Db, config: &Config, rcv: UnboundedReceiver<Event>) -> Result<Self> {
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
            data: std::marker::PhantomData,
            db,
            next_block: chain.last_known_block as u64 + 1,
            chain,
            addresses: config.sync.seed_addresses.clone(),
            cuckoo,
            factory,
            provider,
            buffer: Vec::with_capacity(config.sync.buffer_size),
            buffer_capacity: config.sync.buffer_size,
            rcv,
        })
    }
}

impl BackfillSync {}

trait Sync {
    #[instrument(skip(self), fields(chain_id = self.chain.chain_id))]
    pub async fn run(mut self) -> Result<()> {
        loop {
            self.process_events().await?;

            match self.provider.header_by_number(self.next_block)? {
                // got a block. process it, only flush if needed
                Some(header) => {
                    self.process_block(&header).await?;
                    self.next_block += 1;
                    self.maybe_flush().await?;
                }

                // no block found. take the wait chance to flush, and wait for new block
                None => {
                    self.flush().await?;
                    self.wait_new_block(self.next_block).await?;
                }
            }
        }
    }

    pub async fn process_events(&mut self) -> Result<()> {
        while let Ok(event) = self.rcv.try_recv() {
            match event {
                Event::AccountRegistered { address } => {
                    self.addresses.insert(address);
                    self.cuckoo.insert(&address);
                    self.setup_backfill(address).await?;
                }
            }
        }
        Ok(())
    }

    /// Create a new job for backfilling history for a new account
    /// before the current sync point
    async fn setup_backfill(&mut self, address: Address) -> Result<()> {
        self.db
            .create_backfill_job(
                address.into(),
                self.chain.chain_id,
                (self.next_block - 1) as i32,
                self.chain.start_block,
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
            .update_chain(self.chain.chain_id as u64, self.next_block - 1)
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
