use diesel::pg::Pg;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use super::schema::{accounts, backfill_jobs, chains, txs};
use super::types::{Address, B256};

#[derive(Debug, Queryable, Selectable, Serialize)]
#[diesel(table_name = accounts, check_for_backend(Pg))]
pub struct Account {
    pub address: Address,
    pub chain_id: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Queryable, Selectable, Serialize)]
#[diesel(table_name = txs, check_for_backend(Pg))]
pub struct Txs {
    pub address: Address,
    pub chain_id: i32,
    pub hash: B256,
    pub block_number: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Deserialize, Insertable)]
#[diesel(table_name = txs, check_for_backend(Pg))]
pub struct CreateTx {
    pub address: Address,
    pub chain_id: i32,
    pub hash: B256,
    pub block_number: i32,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = chains, check_for_backend(Pg))]
pub struct Chain {
    pub chain_id: i32,
    pub start_block: i32,
    pub last_known_block: i32,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = backfill_jobs, check_for_backend(Pg))]
pub struct BackfillJob {
    pub addresses: Vec<Address>,
    pub chain_id: i32,
    pub from_block: i32,
    pub to_block: i32,
}
