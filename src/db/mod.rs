pub mod models;
mod schema;
mod types;

use color_eyre::Result;
use diesel::{insert_into, prelude::*, update};
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection, RunQueryDsl,
};
use tracing::{instrument, trace};

use crate::config::{ChainConfig, DbConfig};

use self::models::{Chain, CreateTx, Register};

#[derive(Clone)]
pub struct Db {
    pool: Pool<AsyncPgConnection>,
}

impl Db {
    pub async fn connect(config: &DbConfig) -> Result<Self> {
        let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(config.url.clone());
        let pool = Pool::builder(config).build()?;
        Ok(Self { pool })
    }

    /// Seeds the database with a chain configuration
    /// Skips if the chain already exists
    /// Returns the new or existing chain configuration
    #[instrument(skip(self, chain), fields(chain_id = chain.chain_id, start_block = chain.start_block))]
    pub async fn setup_chain(&self, chain: &ChainConfig) -> Result<Chain> {
        use schema::chains::dsl::*;

        let mut conn = self.pool.get().await?;

        let res = insert_into(chains)
            .values((
                chain_id.eq(chain.chain_id as i32),
                start_block.eq(chain.start_block as i32),
                last_known_block.eq(chain.start_block as i32 - 1),
            ))
            .on_conflict_do_nothing()
            .execute(&mut conn)
            .await;

        handle_error(res).await?;

        let res: Chain = schema::chains::table
            .filter(chain_id.eq(chain.chain_id as i32))
            .select(Chain::as_select())
            .first(&mut conn)
            .await?;

        Ok(res)
    }

    /// Updates the last known block for a chain
    #[instrument(skip(self, id))]
    pub async fn update_chain(&self, id: u64, last_known: u64) -> Result<()> {
        use schema::chains::dsl::*;

        let mut conn = self.pool.get().await?;

        let res = update(chains)
            .filter(chain_id.eq(id as i32))
            .set(last_known_block.eq(last_known as i32))
            .execute(&mut conn)
            .await;

        handle_error(res).await
    }

    #[instrument(skip(self))]
    pub async fn register(&self, register: Register) -> Result<()> {
        use schema::accounts::dsl::*;

        let mut conn = self.pool.get().await?;

        let res = insert_into(accounts)
            .values(&register)
            .execute(&mut conn)
            .await;

        handle_error(res).await
    }

    #[instrument(skip(self, txs), fields(txs = txs.len()))]
    pub async fn create_txs(&self, txs: Vec<CreateTx>) -> Result<()> {
        use schema::txs::dsl::*;

        let mut conn = self.pool.get().await?;

        let res = insert_into(txs)
            .values(&txs)
            .on_conflict_do_nothing()
            .execute(&mut conn)
            .await;

        handle_error(res).await
    }
}

pub async fn handle_error(res: diesel::QueryResult<usize>) -> Result<()> {
    match res {
        Ok(_) => Ok(()),
        Err(diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::ForeignKeyViolation,
            _,
        )) => Ok(()),
        Err(e) => Err(e)?,
    }
}
