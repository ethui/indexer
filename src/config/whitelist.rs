use std::{path::PathBuf, str::FromStr};

use color_eyre::Result;
use reth_primitives::Address;
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug, Default)]
pub struct WhitelistConfig {
    file: Option<PathBuf>,
    whitelist: Option<Vec<Address>>,
}

impl WhitelistConfig {
    pub fn is_whitelisted(&self, addr: &Address) -> bool {
        self.whitelist.as_ref().map_or(false, |w| w.contains(addr))
    }

    pub(super) fn preload(&mut self) -> Result<()> {
        if self.whitelist.is_some() {
            return Ok(());
        }

        if let Some(file) = &self.file {
            // load the file
            let contents = std::fs::read_to_string(file)?;
            let whitelist = contents
                .lines()
                .map(|line| {
                    let addr = line.split_whitespace().next().unwrap();
                    Address::from_str(addr)
                })
                .collect::<Result<Vec<_>, _>>()?;

            self.whitelist = Some(whitelist);
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn for_test(whitelist: Vec<Address>) -> Self {
        Self {
            file: None,
            whitelist: Some(whitelist),
        }
    }
}

#[cfg(test)]
mod test {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn test_preload() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(
            file,
            "0x0063A660Fb166E9deF01C7B4fd0303B054Ed1B9e (ethui.eth)"
        )?;

        let path = file.path().to_path_buf();

        let mut config = WhitelistConfig {
            file: Some(path),
            whitelist: None,
        };

        config.preload()?;

        let expected_addr = Address::from_str("0x0063A660Fb166E9deF01C7B4fd0303B054Ed1B9e")?;
        assert!(config.whitelist.is_some());
        assert_eq!(config.whitelist, Some(vec![expected_addr]));
        assert!(config.is_whitelisted(&expected_addr));

        Ok(())
    }
}
