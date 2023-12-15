#![cfg(test)]
#![allow(dead_code)]

use std::{env, path::PathBuf};

use color_eyre::{eyre::eyre, Result};
use diesel::sql_types::{Bytea, Integer};
use diesel::{sql_query, Connection, PgConnection, RunQueryDsl};
use diesel_migrations::MigrationHarness;
use iron_indexer::db::types::Address;
use iron_indexer::{config::Config, db::MIGRATIONS};

pub fn one_time_setup(config_file: &str) -> Result<()> {
    let url = env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL must be set");
    let mut conn = PgConnection::establish(&url)?;
    let config = Config::read_from(&PathBuf::from(config_file))?;

    conn.run_pending_migrations(MIGRATIONS)
        .map(|_| ())
        .map_err(|e| eyre!("{}", e))?;

    sql_query("TRUNCATE TABLE accounts CASCADE").execute(&mut conn)?;
    sql_query("TRUNCATE TABLE chains CASCADE").execute(&mut conn)?;

    sql_query("INSERT INTO chains (chain_id, start_block, last_known_block) VALUES ($1, $2, $3)")
        .bind::<Integer, _>(config.chain.chain_id)
        .bind::<Integer, _>(0)
        .bind::<Integer, _>(config.chain.start_block as i32)
        .execute(&mut conn)?;

    let addresses: Vec<Address> =
        std::fs::read_to_string("benches/datasets/sepolia-usdc-holders.txt")?
            .lines()
            .take(1000)
            .map(|l| Address(l.parse().unwrap()))
            .collect();

    for address in addresses.iter() {
        sql_query("INSERT INTO accounts (address, chain_id) VALUES ($1, $2)")
            .bind::<Bytea, _>(address)
            .bind::<Integer, _>(config.chain.chain_id)
            .execute(&mut conn)
            .unwrap();
    }

    Ok(())
}

// clear the test database in between each run
// and seeds initial accounts from a file
pub fn setup(config_file: &str) -> Result<(Config, PgConnection)> {
    let url = env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL must be set");
    let mut conn = PgConnection::establish(&url)?;

    sql_query("TRUNCATE TABLE backfill_jobs CASCADE").execute(&mut conn)?;
    sql_query("TRUNCATE TABLE txs CASCADE").execute(&mut conn)?;

    let config = Config::read_from(&PathBuf::from(config_file))?;

    Ok((config, conn))
}
