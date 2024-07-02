mod whitelist;

use std::path::{Path, PathBuf};

use clap::Parser;
use color_eyre::eyre::Result;
use serde::Deserialize;

pub use self::whitelist::WhitelistConfig;

#[derive(Debug, clap::Parser)]
struct Args {
    #[clap(
        long,
        default_value = "ethui-indexer.toml",
        env = "ETHUI_INDEXER_CONFIG"
    )]
    config: PathBuf,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub reth: RethConfig,
    pub chain: ChainConfig,
    pub sync: SyncConfig,

    #[serde(default)]
    pub http: Option<HttpConfig>,

    pub db: DbConfig,
    pub whitelist: WhitelistConfig,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RethConfig {
    pub db: PathBuf,
    pub static_files: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChainConfig {
    pub chain_id: i32,
    #[serde(default = "default_from_block")]
    pub start_block: u64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SyncConfig {
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,

    #[serde(default = "default_backfill_concurrency")]
    pub backfill_concurrency: usize,
}

#[derive(Deserialize, Debug, Clone)]
pub struct HttpConfig {
    #[serde(default = "default_http_port")]
    pub port: u16,

    pub jwt_secret_env: String,
}

impl HttpConfig {
    pub fn jwt_secret(&self) -> String {
        std::env::var(&self.jwt_secret_env).expect("JWT secret not set")
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct DbConfig {
    pub url: String,
}

impl Config {
    pub fn read() -> Result<Self> {
        let args = Args::parse();

        let mut config = Self::read_from(args.config.as_path())?;
        config.whitelist.preload()?;

        Ok(config)
    }

    pub fn read_from(path: &Path) -> Result<Self> {
        Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            port: default_http_port(),
            jwt_secret_env: "ETHUI_JWT_SECRET".to_owned(),
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

fn default_backfill_concurrency() -> usize {
    10
}
