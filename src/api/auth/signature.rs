use color_eyre::{eyre::bail, Result};
use ethers_contract_derive::{Eip712, EthAbiType};
use ethers_core::types::{transaction::eip712::Eip712, Address, Signature};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eip712, EthAbiType, Serialize, Deserialize)]
#[eip712(
    name = "ethui",
    version = "1",
    chain_id = 1,
    verifying_contract = "0x0000000000000000000000000000000000000000"
)]
pub struct IndexerAuth {
    pub(super) address: Address,
    pub(super) valid_until: u64,
}

impl IndexerAuth {
    pub fn new(address: Address, valid_until: u64) -> Self {
        Self {
            address,
            valid_until,
        }
    }

    pub fn check(&self, signature: &Signature) -> Result<()> {
        self.check_expiration()?;
        let hash = self.encode_eip712()?;
        signature.verify(hash, self.address)?;

        Ok(())
    }

    fn check_expiration(&self) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        if self.valid_until <= now {
            bail!("signature timestamp has expired");
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {

    use color_eyre::Result;
    use ethers_core::types::{
        transaction::eip712::{Eip712, TypedData},
        Address,
    };
    use rstest::rstest;

    use super::*;
    use crate::api::test_utils::{address, now, sign_typed_data};

    #[rstest]
    #[tokio::test]
    async fn check_signature(address: Address, now: u64) -> Result<()> {
        let data: IndexerAuth = IndexerAuth::new(address, now + 20);
        let signature = sign_typed_data(&data).await?;

        data.check(&signature)?;
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
            "IndexerAuth": [
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
          "primaryType": "IndexerAuth",
          "domain": {
            "name": "ethui",
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

        let data: IndexerAuth = IndexerAuth::new(address, valid_until);
        let hash = data.encode_eip712()?;

        assert_eq!(expected_hash, hash);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn check_fails_with_expired_timestamp(address: Address, now: u64) -> Result<()> {
        let data: IndexerAuth = IndexerAuth::new(address, now - 20);

        assert!(data.check_expiration().is_err());
        Ok(())
    }
}
