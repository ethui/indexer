pub mod models;
mod schema;
mod types;

use color_eyre::Result;
use diesel::insert_into;
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection, RunQueryDsl,
};
use tracing::instrument;

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

    #[instrument(skip(self))]
    pub async fn register(&self, register: Register) -> Result<()> {
        let mut conn = self.pool.get().await?;

        insert_into(accounts::dsl::accounts)
            .values(&register)
            .execute(&mut conn)
            .await?;

        Ok(())
    }

    #[instrument(skip(self, txs), fields(txs = txs.len()))]
    pub async fn create_txs(&self, txs: Vec<CreateTx>) -> Result<()> {
        let mut conn = self.pool.get().await?;

        let res = insert_into(txs::dsl::txs)
            .values(&txs)
            .on_conflict_do_nothing()
            .execute(&mut conn)
            .await;

        match res {
            Ok(_) => Ok(()),
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::ForeignKeyViolation,
                _,
            )) => Ok(()),
            Err(e) => Err(e)?,
        }
    }
}
