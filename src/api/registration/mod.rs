use color_eyre::{eyre::eyre, Result};
use reth_primitives::{Address, TxHash};
use reth_provider::ReceiptProvider as _;
use serde::{Deserialize, Serialize};

use super::app_state::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegistrationProof {
    Whitelist,
    TxHash(TxHash),

    #[cfg(test)]
    Test,
}

#[allow(unused)]
impl RegistrationProof {
    pub async fn validate(&self, address: Address, state: &AppState) -> Result<()> {
        match self {
            Self::Whitelist => {
                if !state.config.whitelist.is_whitelisted(&address) {
                    return Err(eyre!("Not Whitelisted"));
                }
            }

            Self::TxHash(hash) => {
                let provider = state.provider_factory.get()?;
                let receipt = provider.receipt_by_hash(*hash)?;
                dbg!(receipt);
                todo!()
            }

            #[cfg(test)]
            Self::Test => return Ok(()),
        };

        Ok(())
    }
}
