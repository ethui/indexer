pub mod models;
mod schema;
mod types;

use color_eyre::Result;
use diesel::insert_into;
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection, RunQueryDsl,
};

use crate::config::DbConfig;

use self::models::{CreateTx, Register};
use self::schema::{accounts, txs};

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

    pub async fn register(&self, register: Register) -> Result<()> {
        let mut conn = self.pool.get().await?;

        insert_into(accounts::dsl::accounts)
            .values(&register)
            .execute(&mut conn)
            .await?;

        Ok(())
    }

    pub async fn create_tx(&self, tx: CreateTx) -> Result<()> {
        let mut conn = self.pool.get().await?;

        insert_into(txs::dsl::txs)
            .values(&tx)
            .execute(&mut conn)
            .await?;

        Ok(())
    }
}
