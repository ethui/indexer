use color_eyre::eyre::{self, Result};
use reth_db::{open_db_read_only, DatabaseEnv};
use std::ops::Range;

use reth_db::mdbx::{tx::Tx, RO};
use reth_primitives::{Header, Receipt, TransactionSignedNoHash};
use reth_provider::{
    BlockNumReader, BlockReader, DatabaseProvider, HeaderProvider, ProviderFactory,
    ReceiptProvider, TransactionsProvider,
};

use crate::{config::Config, db::models::Chain};

/// Wraps a provider to access Reth DB
/// While the indexer is heavily coupled to this particular provider,
/// it still benefits from abstracting it so it can be swapped out for testing purposes
pub struct RethDBProvider {
    /// Reth Provider factory
    factory: ProviderFactory<DatabaseEnv>,

    /// Current Reth DB provider
    provider: DatabaseProvider<Tx<RO>>,
}

pub trait Provider: Sized + Send {
    /// Creates a new provider
    fn new(config: &Config, chain: &Chain) -> Result<Self>;

    /// Reloads the provider
    /// This is necessary when Reth receives a new block
    fn reload(&mut self) -> Result<()>;

    /// Returns the last block number
    fn last_block_number(&self) -> Result<u64>;

    /// Returns a block header by number
    fn block_header(&self, number: u64) -> Result<Option<Header>>;

    /// Returns the range of transaction IDs for a block
    fn block_tx_id_ranges(&self, number: u64) -> Result<Range<u64>>;

    /// Returns a transaction by ID
    fn tx_by_id(&self, tx_id: u64) -> Result<Option<TransactionSignedNoHash>>;

    /// Returns a receipt by ID
    fn receipt_by_id(&self, tx_id: u64) -> Result<Option<Receipt>>;
}

impl Provider for RethDBProvider {
    /// Creates a new Reth DB provider
    fn new(config: &Config, chain: &Chain) -> Result<Self> {
        let chain_id = chain.chain_id as u64;
        let config = &config.reth;
        let db = open_db_read_only(&config.db, None)?;

        let spec = match chain_id {
            1 => (*reth_primitives::MAINNET).clone(),
            11155111 => (*reth_primitives::SEPOLIA).clone(),
            _ => return Err(eyre::eyre!("unsupported chain id {}", chain_id)),
        };

        let factory: ProviderFactory<reth_db::DatabaseEnv> = ProviderFactory::new(db, spec);

        let provider: reth_provider::DatabaseProvider<Tx<RO>> = factory.provider()?;
        Ok(Self { factory, provider })
    }

    /// Reloads the provider
    /// This is necessary when Reth receives a new block
    fn reload(&mut self) -> Result<()> {
        self.provider = self.factory.provider()?;
        Ok(())
    }

    /// Returns the last block number
    fn last_block_number(&self) -> Result<u64> {
        Ok(self.provider.last_block_number()?)
    }

    /// Returns a block header by number
    fn block_header(&self, number: u64) -> Result<Option<Header>> {
        Ok(self.provider.header_by_number(number)?)
    }

    /// Returns the range of transaction IDs for a block
    fn block_tx_id_ranges(&self, number: u64) -> Result<Range<u64>> {
        let indices = match self.provider.block_body_indices(number)? {
            Some(indices) => indices,
            None => return Ok(Default::default()),
        };

        Ok(indices.first_tx_num..indices.first_tx_num + indices.tx_count)
    }

    /// Returns a transaction by ID
    fn tx_by_id(&self, tx_id: u64) -> Result<Option<TransactionSignedNoHash>> {
        Ok(self.provider.transaction_by_id_no_hash(tx_id)?)
    }

    /// Returns a receipt by ID
    fn receipt_by_id(&self, tx_id: u64) -> Result<Option<Receipt>> {
        Ok(self.provider.receipt(tx_id)?)
    }
}
