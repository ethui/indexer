use color_eyre::eyre::{self, Result};
use reth_db::{open_db_read_only, DatabaseEnv};

use reth_db::mdbx::{tx::Tx, RO};
use reth_provider::{DatabaseProvider, ProviderFactory};

use crate::{config::Config, db::models::Chain};

/// Wraps a provider to access Reth DB
/// While the indexer is heavily coupled to this particular provider,
/// it still benefits from abstracting it so it can be swapped out for testing purposes
#[derive(Debug)]
pub struct RethProviderFactory {
    /// Reth Provider factory
    factory: ProviderFactory<DatabaseEnv>,
}

impl RethProviderFactory {
    /// Creates a new Reth DB provider
    pub fn new(config: &Config, chain: &Chain) -> Result<Self> {
        let chain_id = chain.chain_id as u64;
        let config = &config.reth;
        let db = open_db_read_only(&config.db, Default::default())?;

        let spec = match chain_id {
            1 => (*reth_primitives::MAINNET).clone(),
            11155111 => (*reth_primitives::SEPOLIA).clone(),
            _ => return Err(eyre::eyre!("unsupported chain id {}", chain_id)),
        };

        let factory: ProviderFactory<reth_db::DatabaseEnv> =
            ProviderFactory::new(db, spec, config.static_files.clone())?;

        Ok(Self { factory })
    }

    pub fn get(&self) -> Result<DatabaseProvider<Tx<RO>>> {
        Ok(self.factory.provider()?)
    }
}
