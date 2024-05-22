#![cfg(test)]

use std::str::FromStr;

use color_eyre::Result;
use ethers_core::types::{Address, Signature};
use ethers_signers::{coins_bip39::English, MnemonicBuilder, Signer};

use crate::api::auth::IndexerAuth;

#[rstest::fixture]
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[rstest::fixture]
pub fn address() -> Address {
    Address::from_str("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266").unwrap()
}

pub async fn sign_typed_data(data: &IndexerAuth) -> Result<Signature> {
    let mnemonic = String::from("test test test test test test test test test test test junk");
    let derivation_path = String::from("m/44'/60'/0'/0");
    let current_path = format!("{}/{}", derivation_path, 0);
    let chain_id = 1_u32;
    let signer = MnemonicBuilder::<English>::default()
        .phrase(mnemonic.as_ref())
        .derivation_path(&current_path)?
        .build()
        .map(|v| v.with_chain_id(chain_id))?;

    let signature = signer.sign_typed_data(data).await?;

    Ok(signature)
}
