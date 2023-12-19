use color_eyre::{eyre::bail, eyre::eyre, Result};
use ethers_contract_derive::{Eip712, EthAbiType};
use ethers_core::types::Signature;
use ethers_core::types::{transaction::eip712::Eip712, Address};
use serde::Deserialize;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, Eip712, EthAbiType, Deserialize)]
#[eip712(
    name = "IndexSignature",
    version = "1",
    chain_id = 1,
    verifying_contract = "0x0000000000000000000000000000000000000000"
)]
pub struct SignatureData {
    address: Address,
    current_timestamp: u64,
    expiration_timestamp: u64,
}

impl SignatureData {
    pub fn new(address: Address, current_timestamp: u64, expiration_timestamp: u64) -> Self {
        Self {
            address,
            current_timestamp,
            expiration_timestamp,
        }
    }
}

#[derive(Debug, Error, PartialEq)]
enum TimeError {
    #[error("Invalid current timestamp")]
    InvalidCurrentTimestamp,
    #[error("Invalid expiration timestamp")]
    InvalidExpirationTimestamp,
}

pub fn check_time(current_timestamp: u64, expiration_timestamp: u64) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();

    let five_minutes_ago = now - 5 * 60;

    if (current_timestamp < five_minutes_ago) || (current_timestamp > now) {
        bail!(TimeError::InvalidCurrentTimestamp);
    }

    if expiration_timestamp <= now {
        bail!(TimeError::InvalidExpirationTimestamp);
    }

    Ok(())
}

pub fn check_type_data(
    signature: &str,
    address: Address,
    current_timestamp: u64,
    expiration_timestamp: u64,
) -> Result<()> {
    check_time(current_timestamp, expiration_timestamp)?;

    let signature = Signature::from_str(signature)?;
    let data: SignatureData = SignatureData::new(address, current_timestamp, expiration_timestamp);

    let encoded = data.encode_eip712()?;

    signature
        .verify(encoded, address)
        .map_err(|_| eyre!("Signature verification failed"))?;

    Ok(())
}

#[cfg(test)]
pub mod test_utils {

    use crate::api::auth::signature::SignatureData;
    use color_eyre::Result;
    use ethers_core::types::Signature;
    use ethers_signers::{coins_bip39::English, MnemonicBuilder, Signer};

    pub async fn sign_type_data(data: SignatureData) -> Result<Signature> {
        let mnemonic = String::from("test test test test test test test test test test test junk");
        let derivation_path = String::from("m/44'/60'/0'/0");
        let current_path = format!("{}/{}", derivation_path, 0);
        let chain_id = 1_u32;
        let signer = MnemonicBuilder::<English>::default()
            .phrase(mnemonic.as_ref())
            .derivation_path(&current_path)?
            .build()
            .map(|v| v.with_chain_id(chain_id))?;

        let signature = signer.sign_typed_data(&data).await?;

        Ok(signature)
    }
}

#[cfg(test)]
mod test {
    use super::test_utils;
    use super::*;
    use ethers_core::types::transaction::eip712::TypedData;

    #[tokio::test]
    async fn test_signature() -> Result<()> {
        let current_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let expiration_timestamp = current_timestamp + 20 * 60;
        let address: Address =
            Address::from_str("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266").unwrap();

        let data: SignatureData =
            SignatureData::new(address, current_timestamp, expiration_timestamp);

        let signature = test_utils::sign_type_data(data).await?.to_string();

        check_type_data(&signature, address, current_timestamp, expiration_timestamp)?;
        Ok(())
    }

    #[tokio::test]
    async fn test_encoding() -> Result<()> {
        let json = serde_json::json!( {
          "types": {
            "EIP712Domain": [
              {
                "name": "name",
                "type": "string"
              },
              {
                "name": "version",
                "type": "string"
              },
              {
                "name": "chainId",
                "type": "uint256"
              },
              {
                "name": "verifyingContract",
                "type": "address"
              }
            ],
            "SignatureData": [
              {
                "name": "address",
                "type": "address"
              },
              {
                "name": "currentTimestamp",
                "type": "uint64"
              },
              {
                "name": "expirationTimestamp",
                "type": "uint64"
              }
            ]
          },
          "primaryType": "SignatureData",
          "domain": {
            "name": "IndexSignature",
            "version": "1",
            "chainId": "1",
            "verifyingContract": "0x0000000000000000000000000000000000000000",
          },
          "message": {
            "address": "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
            "currentTimestamp": 1,
            "expirationTimestamp": 2
          }
        });

        let typed_data: TypedData = serde_json::from_value(json).unwrap();
        let hash = typed_data.encode_eip712().unwrap();

        let current_timestamp = 1;
        let expiration_timestamp = 2;
        let address: Address =
            Address::from_str("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266").unwrap();

        let data: SignatureData =
            SignatureData::new(address, current_timestamp, expiration_timestamp);

        let encoded = data.encode_eip712()?;

        assert_eq!(encoded, hash);
        Ok(())
    }

    #[tokio::test]
    async fn test_check_time_invalid_current_time() -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        let initial_time = 0;
        let five_minutes = 5 * 60;
        let ten_minutes = 10 * 60;
        let ten_minutes_ago = now - ten_minutes;
        let five_minutes_after = now + five_minutes;
        let ten_minutes_after = now + ten_minutes;

        assert_eq!(
            check_time(initial_time, five_minutes)
                .unwrap_err()
                .downcast::<TimeError>()?,
            TimeError::InvalidCurrentTimestamp
        );

        assert_eq!(
            check_time(initial_time, now)
                .unwrap_err()
                .downcast::<TimeError>()?,
            TimeError::InvalidCurrentTimestamp
        );
        assert_eq!(
            check_time(initial_time, ten_minutes_after)
                .unwrap_err()
                .downcast::<TimeError>()?,
            TimeError::InvalidCurrentTimestamp
        );

        assert_eq!(
            check_time(ten_minutes_ago, ten_minutes_after)
                .unwrap_err()
                .downcast::<TimeError>()?,
            TimeError::InvalidCurrentTimestamp
        );

        assert_eq!(
            check_time(five_minutes_after, ten_minutes_after)
                .unwrap_err()
                .downcast::<TimeError>()?,
            TimeError::InvalidCurrentTimestamp
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_check_time_invalid_expiration_timestamp() -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        let five_minutes = 5 * 60;
        let ten_minutes = 10 * 60;
        let five_minutes_ago = now - five_minutes + 10;
        let ten_minutes_ago = now - ten_minutes;

        assert_eq!(
            check_time(now, now).unwrap_err().downcast::<TimeError>()?,
            TimeError::InvalidExpirationTimestamp
        );

        assert_eq!(
            check_time(now, five_minutes_ago)
                .unwrap_err()
                .downcast::<TimeError>()?,
            TimeError::InvalidExpirationTimestamp
        );

        assert_eq!(
            check_time(five_minutes_ago, now)
                .unwrap_err()
                .downcast::<TimeError>()?,
            TimeError::InvalidExpirationTimestamp
        );

        assert_eq!(
            check_time(five_minutes_ago, ten_minutes_ago)
                .unwrap_err()
                .downcast::<TimeError>()?,
            TimeError::InvalidExpirationTimestamp
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_check_time_valid_timestamps() -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        let five_minutes = 5 * 60;
        let ten_minutes = 10 * 60;
        let five_minutes_ago = now - five_minutes + 10;
        let five_minutes_after = now + five_minutes;
        let ten_minutes_after = now + ten_minutes;

        assert!(check_time(five_minutes_ago, ten_minutes_after).is_ok());
        assert!(check_time(now, five_minutes_after).is_ok());

        Ok(())
    }
}
