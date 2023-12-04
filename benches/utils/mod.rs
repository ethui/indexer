use std::{env, path::PathBuf};

use color_eyre::{eyre::eyre, Result};
use diesel::{sql_query, Connection, PgConnection, RunQueryDsl};
use diesel_migrations::MigrationHarness;
use iron_indexer::{config::Config, db::MIGRATIONS};

// clear the test database in between each run
// and seeds initial accounts from a file
pub fn setup(config_file: &str) -> Result<(Config, PgConnection)> {
    let url = env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL must be set");
    let mut conn = PgConnection::establish(&url)?;

    conn.run_pending_migrations(MIGRATIONS)
        .map(|_| ())
        .map_err(|e| eyre!("{}", e))?;
    sql_query("TRUNCATE TABLE backfill_jobs CASCADE").execute(&mut conn)?;
    sql_query("TRUNCATE TABLE txs CASCADE").execute(&mut conn)?;
    sql_query("TRUNCATE TABLE accounts CASCADE").execute(&mut conn)?;
    sql_query("TRUNCATE TABLE chains CASCADE").execute(&mut conn)?;

    let config = Config::read_from(&PathBuf::from(config_file))?;

    Ok((config, conn))
}
