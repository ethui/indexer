pub mod models;
mod schema;
pub mod types;

use color_eyre::{eyre::eyre, Result};
use diesel::{delete, insert_into, prelude::*, update};
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    scoped_futures::ScopedFutureExt,
    AsyncConnection, AsyncPgConnection, RunQueryDsl,
};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tokio::sync::mpsc::UnboundedSender;
use tracing::instrument;

use self::{
    models::{Chain, CreateTx},
    types::Address,
};
use crate::{
    config::{ChainConfig, Config},
    db::models::{BackfillJob, BackfillJobWithChainId, BackfillJobWithId},
};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// An abstract DB connection
/// In production, `PgBackend` is meant to be used, but the trait allows for the existance of
/// `InMemoryBackend` as well, which is useful for testing
#[derive(Clone)]
pub struct Db {
    /// async db pool
    pool: Pool<AsyncPgConnection>,

    /// notify sync job of new accounts
    new_accounts_tx: Option<UnboundedSender<alloy_primitives::Address>>,

    /// notify backfill job of new jobs
    /// (which are created from new accounts, but asynchronously, so need their own event)
    /// payload is empty because the job only needs a notification to rearrange from DB data
    new_job_tx: Option<UnboundedSender<()>>,

    /// chain ID we're running on
    chain_id: i32,
}

impl Db {
    pub async fn connect(
        config: &Config,
        new_accounts_tx: UnboundedSender<alloy_primitives::Address>,
        new_job_tx: UnboundedSender<()>,
    ) -> Result<Self> {
        Self::migrate(&config.db.url).await?;

        let db_config =
            AsyncDieselConnectionManager::<AsyncPgConnection>::new(config.db.url.clone());
        let pool = Pool::builder(db_config).build()?;

        Ok(Self {
            pool,
            new_accounts_tx: Some(new_accounts_tx),
            new_job_tx: Some(new_job_tx),
            chain_id: config.chain.chain_id,
        })
    }

    #[cfg(test)]
    pub async fn connect_test() -> Result<Self> {
        let db_url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL not set");
        Self::migrate(&db_url).await?;
        let db_config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(db_url);
        let pool = Pool::builder(db_config).build()?;

        let res = Self {
            pool,
            new_accounts_tx: None,
            new_job_tx: None,
            chain_id: 31337,
        };

        res.truncate().await?;
        Ok(res)
    }

    #[instrument(skip(url))]
    async fn migrate(url: &str) -> Result<()> {
        let url = url.to_owned();

        tokio::task::spawn_blocking(move || {
            let mut conn = PgConnection::establish(&url).expect("Failed to connect to DB");
            conn.run_pending_migrations(MIGRATIONS)
                .map(|_| ())
                .map_err(|e| eyre!("{}", e))
        })
        .await??;

        Ok(())
    }

    /// Truncate all tables
    /// to be called before each unit test
    #[cfg(test)]
    async fn truncate(&self) -> Result<()> {
        use diesel::sql_query;

        let mut conn = self.pool.get().await?;
        for table in ["backfill_jobs"].iter() {
            sql_query(format!("TRUNCATE TABLE {} CASCADE", table))
                .execute(&mut conn)
                .await
                .unwrap();
        }
        Ok(())
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
        if let (Ok(_), Some(tx)) = (&res, &self.new_accounts_tx) {
            tx.send(address.0)?;
        }

        handle_error(res).await
    }

    pub async fn get_addresses(&self) -> Result<Vec<Address>> {
        use schema::accounts::dsl;
        let mut conn = self.pool.get().await?;

        let res = dsl::accounts
            .filter(dsl::chain_id.eq(self.chain_id))
            .select(dsl::address)
            .load(&mut conn)
            .await?;
        Ok(res)
    }

    #[instrument(skip(self, txs), fields(txs = txs.len()))]
    pub async fn create_txs(&self, txs: Vec<CreateTx>) -> Result<()> {
        use schema::txs::dsl;
        let mut conn = self.pool.get().await?;

        let res = insert_into(dsl::txs)
            .values(&txs)
            .on_conflict_do_nothing()
            .execute(&mut conn)
            .await;

        handle_error(res).await
    }

    #[instrument(skip(self))]
    pub async fn create_backfill_job(&self, address: Address, low: i32, high: i32) -> Result<()> {
        use schema::backfill_jobs::dsl;
        let mut conn = self.pool.get().await?;

        let res = insert_into(dsl::backfill_jobs)
            .values((
                dsl::addresses.eq(vec![address]),
                dsl::chain_id.eq(self.chain_id),
                dsl::low.eq(low),
                dsl::high.eq(high),
            ))
            .on_conflict_do_nothing()
            .execute(&mut conn)
            .await;

        // notify backfill job new work is available
        if let (Ok(_), Some(tx)) = (&res, &self.new_job_tx) {
            tx.send(())?;
        }

        handle_error(res).await
    }

    pub async fn get_backfill_jobs(&self) -> Result<Vec<BackfillJobWithId>> {
        use schema::backfill_jobs::dsl;
        let mut conn = self.pool.get().await?;

        let res = dsl::backfill_jobs
            .filter(dsl::chain_id.eq(self.chain_id))
            .select(BackfillJobWithId::as_select())
            .order(dsl::high.desc())
            .load(&mut conn)
            .await?;

        Ok(res)
    }

    /// Deletes all existing backfill jobs, and rearranges them for optimal I/O
    /// See `utils::rearrange` for more details
    #[instrument(skip(self))]
    pub async fn reorg_backfill_jobs(&self) -> Result<()> {
        use schema::backfill_jobs::dsl;
        let mut conn = self.pool.get().await?;

        conn.transaction::<_, diesel::result::Error, _>(|mut conn| {
            async move {
                let jobs = dsl::backfill_jobs
                    .filter(dsl::chain_id.eq(self.chain_id))
                    .select(BackfillJob::as_select())
                    .order(dsl::high.desc())
                    .load(&mut conn)
                    .await?;

                let rearranged = crate::rearrange::rearrange(&jobs);

                delete(dsl::backfill_jobs).execute(&mut conn).await?;

                let rearranged: Vec<_> = rearranged
                    .into_iter()
                    .map(|j| BackfillJobWithChainId {
                        addresses: j.addresses,
                        chain_id: self.chain_id,
                        low: j.low,
                        high: j.high,
                    })
                    .collect();

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

    /// Updates the to_block for a backfill job
    pub async fn update_job(&self, id: i32, high: u64) -> Result<()> {
        use schema::backfill_jobs::dsl;
        let mut conn = self.pool.get().await?;

        let res = update(dsl::backfill_jobs)
            .filter(dsl::id.eq(id))
            .set(dsl::high.eq(high as i32))
            .execute(&mut conn)
            .await;
        handle_error(res).await
    }
}

async fn handle_error(res: diesel::QueryResult<usize>) -> Result<()> {
    match res {
        Ok(_) => Ok(()),
        Err(diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::ForeignKeyViolation,
            _,
        )) => Ok(()),
        Err(e) => Err(e)?,
    }
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db")
            .field("chain_id", &self.chain_id)
            .finish()
    }
}
