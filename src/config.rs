use std::{collections::BTreeSet, path::PathBuf};

use alloy_primitives::Address;
use clap::Parser;
use color_eyre::eyre::Result;
use serde::Deserialize;

#[derive(Debug, clap::Parser)]
struct Args {
    #[clap(long, default_value = "iron-indexer.toml", env = "IRON_INDEXER_CONFIG")]
    config: PathBuf,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub reth: RethConfig,
    pub chain: ChainConfig,
    pub sync: SyncConfig,

    #[serde(default)]
    pub http: HttpConfig,

    pub db: DbConfig,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RethConfig {
    pub db: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChainConfig {
    pub chain_id: i32,
    #[serde(default = "default_from_block")]
    pub start_block: u64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SyncConfig {
    pub seed_addresses: BTreeSet<Address>,

    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,
}

#[derive(Deserialize, Debug, Clone)]
pub struct HttpConfig {
    #[serde(default = "default_http_port")]
    pub port: u16,
}

#[derive(Deserialize, Clone, Debug)]
pub struct DbConfig {
    pub url: String,
}

impl Config {
    pub fn read() -> Result<Self> {
        let args = Args::parse();

        Ok(toml::from_str(&std::fs::read_to_string(
            args.config.as_path(),
        )?)?)
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            port: default_http_port(),
        }
    }
}

fn default_from_block() -> u64 {
    1
}

fn default_http_port() -> u16 {
    9500
}

fn default_buffer_size() -> usize {
    1000
}
