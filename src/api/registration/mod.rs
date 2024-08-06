use color_eyre::{eyre::eyre, Result};
use reth_primitives::{Address, TransactionSigned, TxHash};
use reth_provider::TransactionsProvider as _;
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
                match provider.transaction_by_hash(*hash)? {
                    Some(tx) => self.validate_tx(address, state, &tx)?,
                    None => return Err(eyre!("Transaction not found")),
                }
            }

            #[cfg(test)]
            Self::Test => return Ok(()),
        };

        Ok(())
    }

    fn validate_tx(
        &self,
        address: Address,
        state: &AppState,
        tx: &TransactionSigned,
    ) -> Result<()> {
        if tx.recover_signer() != Some(address) {
            return Err(eyre!("Transaction origin does not match given address"));
        }

        let Some(payment_config) = state.config.payment else {
            return Ok(());
        };

        if tx.to() != Some(payment_config.address) {
            return Err(eyre!("Transaction must be sent to the payment address"));
        }

        if tx.value() < payment_config.min_amount {
            return Err(eyre!(
                "Transaction value must be at least {}",
                payment_config.min_amount
            ));
        }

        Ok(())
    }
}
