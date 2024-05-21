use std::str::FromStr;

use color_eyre::{eyre::bail, Result};
use ethers_contract_derive::{Eip712, EthAbiType};
use ethers_core::types::{transaction::eip712::Eip712, Address, Signature};
use serde::Deserialize;

#[derive(Debug, Clone, Eip712, EthAbiType, Deserialize)]
#[eip712(
    name = "IndexAuth",
    version = "1",
    chain_id = 1,
    verifying_contract = "0x0000000000000000000000000000000000000000"
)]
pub struct SignatureData {
    address: Address,
    valid_until: u64,
}

impl SignatureData {
    pub fn new(address: Address, valid_until: u64) -> Self {
        Self {
            address,
            valid_until,
        }
    }
}

pub fn check_expiration(valid_until: u64) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();

    if valid_until <= now {
        bail!("signature timestamp has expired");
    }

    Ok(())
}

pub fn check_type_data(signature: &str, address: Address, valid_until: u64) -> Result<()> {
    check_expiration(valid_until)?;

    let signature = Signature::from_str(signature)?;
    let data: SignatureData = SignatureData::new(address, valid_until);

    let encoded = data.encode_eip712()?;

    signature.verify(encoded, address)?;

    Ok(())
}

#[cfg(test)]
mod test {

    use color_eyre::Result;
    use ethers_core::types::{
        transaction::eip712::{Eip712, TypedData},
        Address,
    };
    use rstest::rstest;

    use crate::api::{
        auth::signature::{check_expiration, check_type_data, SignatureData},
        test_utils::{address, now, sign_typed_data},
    };

    #[rstest]
    #[tokio::test]
    async fn test_signature(address: Address, now: u64) -> Result<()> {
        let valid_until = now + 20 * 60;

        let data: SignatureData = SignatureData::new(address, valid_until);
        let signature = sign_typed_data(data).await?.to_string();

        check_type_data(&signature, address, valid_until)?;
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_encoding(address: Address, now: u64) -> Result<()> {
        let valid_until = now + 5 * 60;

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
                "name": "validUntil",
                "type": "uint64"
              }
            ]
          },
          "primaryType": "SignatureData",
          "domain": {
            "name": "IndexAuth",
            "version": "1",
            "chainId": "1",
            "verifyingContract": "0x0000000000000000000000000000000000000000",
          },
          "message": {
            "address": format!("0x{:x}",address),
            "validUntil": valid_until
          }
        });

        let expected_data: TypedData = serde_json::from_value(json).unwrap();
        let expected_hash = expected_data.encode_eip712()?;

        let data: SignatureData = SignatureData::new(address, valid_until);
        let hash = data.encode_eip712()?;

        assert_eq!(expected_hash, hash);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_check_time_invalid_current_time(now: u64) -> Result<()> {
        assert!(check_expiration(now - 1).is_err());
        Ok(())
    }
}
