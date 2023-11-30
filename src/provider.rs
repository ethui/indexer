use color_eyre::eyre::{self, Result};
use reth_db::{open_db_read_only, DatabaseEnv};
use reth_provider::ProviderFactory;

use crate::config::RethConfig;

pub fn provider_factory(
    chain_id: u64,
    config: &RethConfig,
) -> Result<ProviderFactory<DatabaseEnv>> {
    let db = open_db_read_only(&config.db, None)?;

    let spec = match chain_id {
        1 => (*reth_primitives::MAINNET).clone(),
        11155111 => (*reth_primitives::SEPOLIA).clone(),
        _ => return Err(eyre::eyre!("unsupported chain id {}", chain_id)),
    };

    let factory: ProviderFactory<reth_db::DatabaseEnv> = ProviderFactory::new(db, spec);

    Ok(factory)
}
