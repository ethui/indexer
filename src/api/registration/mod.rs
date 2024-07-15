use color_eyre::Result;
use ethers_core::types::Address;
use reth_primitives::TxHash;
use serde::{Deserialize, Serialize};

use crate::db::Db;

#[derive(Debug, Serialize, Deserialize)]
pub enum RegistrationProof {
    Whitelist,
    TxHash(TxHash),

    #[cfg(test)]
    Test,
}

#[allow(unused)]
impl RegistrationProof {
    pub async fn validate(&self, address: Address, db: &Db) -> Result<()> {
        match self {
            Self::Whitelist => {
                todo!()
            }
            Self::TxHash(_hash) => {
                todo!()
            }

            #[cfg(test)]
            Self::Test => return Ok(()),
        };
    }
}
