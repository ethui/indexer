pub mod models;
mod schema;
mod types;
mod utils;

use color_eyre::Result;
use diesel::{delete, insert_into, prelude::*, update};
use diesel_async::scoped_futures::ScopedFutureExt;
use diesel_async::AsyncConnection;
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection, RunQueryDsl,
};
use tokio::sync::mpsc::UnboundedSender;
use tracing::instrument;

use crate::config::{ChainConfig, Config};
use crate::db::models::BackfillJob;
use crate::events::Event;

use self::models::{Chain, CreateTx};
use self::types::Address;

#[derive(Clone)]
pub struct Db {
    pool: Pool<AsyncPgConnection>,
    tx: UnboundedSender<Event>,
    chain_id: i32,
}

impl Db {
    pub async fn connect(config: &Config, tx: UnboundedSender<Event>) -> Result<Self> {
        let db_config =
            AsyncDieselConnectionManager::<AsyncPgConnection>::new(config.db.url.clone());
        let pool = Pool::builder(db_config).build()?;
        Ok(Self {
            pool,
            tx,
            chain_id: config.chain.chain_id,
        })
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
                chain_id.eq(chain.chain_id),
                start_block.eq(chain.start_block as i32),
                last_known_block.eq(chain.start_block as i32 - 1),
            ))
            .on_conflict_do_nothing()
            .execute(&mut conn)
            .await;

        handle_error(res).await?;

        let res: Chain = schema::chains::table
            .filter(chain_id.eq(chain.chain_id))
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

    /// Register a new account
    #[instrument(skip(self))]
    pub async fn register(&self, address: Address) -> Result<()> {
        use schema::accounts::dsl;

        let mut conn = self.pool.get().await?;

        let res = insert_into(dsl::accounts)
            .values((dsl::address.eq(&address), dsl::chain_id.eq(self.chain_id)))
            .execute(&mut conn)
            .await;

        // notify sync job if creation was successful
        if res.is_ok() {
            self.tx
                .send(Event::AccountRegistered { address: address.0 })?;
        }

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

    #[instrument(skip(self))]
    pub async fn create_backfill_job(
        &self,
        address: Address,
        chain_id: i32,
        from_block: i32,
        to_block: i32,
    ) -> Result<()> {
        use schema::backfill_jobs::dsl;
        let mut conn = self.pool.get().await?;

        let res = insert_into(dsl::backfill_jobs)
            .values((
                dsl::address.eq(address),
                dsl::chain_id.eq(chain_id),
                dsl::from_block.eq(from_block),
                dsl::to_block.eq(to_block),
            ))
            .on_conflict_do_nothing()
            .execute(&mut conn)
            .await;

        handle_error(res).await
    }

    pub async fn get_backfill_jobs(&self) -> Result<Vec<BackfillJob>> {
        use schema::backfill_jobs::dsl;
        let mut conn = self.pool.get().await?;

        let res = dsl::backfill_jobs
            .filter(dsl::chain_id.eq(self.chain_id))
            .select(BackfillJob::as_select())
            .order(dsl::from_block.desc())
            .load(&mut conn)
            .await?;

        Ok(res)
    }

    /// Deletes all existing backfill jobs, and rearranges them for optimal I/O
    /// See `utils::rearrange` for more details
    pub async fn rearrange_backfill_jobs(&self) -> Result<()> {
        use schema::backfill_jobs::dsl;
        let mut conn = self.pool.get().await?;

        conn.transaction::<_, diesel::result::Error, _>(|mut conn| {
            async move {
                let jobs = dsl::backfill_jobs
                    .filter(dsl::chain_id.eq(self.chain_id))
                    .select(BackfillJob::as_select())
                    .order(dsl::from_block.desc())
                    .load(&mut conn)
                    .await?;

                let rearranged = utils::rearrange(jobs, self.chain_id);

                delete(dsl::backfill_jobs).execute(&mut conn).await?;

                insert_into(dsl::backfill_jobs)
                    .values(&rearranged)
                    .execute(&mut conn)
                    .await?;

                Ok(())
            }
            .scope_boxed()
        })
        .await?;

        Ok(())
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
