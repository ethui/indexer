use color_eyre::{eyre::eyre, Result};
use reth_primitives::{Address, TxHash};
use serde::{Deserialize, Serialize};

use crate::{config::Config, db::Db};

#[derive(Debug, Serialize, Deserialize)]
pub enum RegistrationProof {
    Whitelist,
    TxHash(TxHash),

    #[cfg(test)]
    Test,
}

#[allow(unused)]
impl RegistrationProof {
    pub async fn validate(&self, address: Address, db: &Db, config: &Config) -> Result<()> {
        match self {
            Self::Whitelist => {
                if !config.whitelist.is_whitelisted(&address) {
                    return Err(eyre!("Not Whitelisted"));
                }
                todo!()
            }
            Self::TxHash(_hash) => {
                todo!()
            }

            #[cfg(test)]
            Self::Test => return Ok(()),
        };

        Ok(())
    }
}
