mod worker;

use color_eyre::Result;
use tokio::sync::Semaphore;
use worker::Worker;

use crate::db::Db;

pub struct Backfill {
    pub db: Db,
    pub semaphore: Semaphore,
    pub workers: Vec<Worker>,
}

impl Backfill {
    pub async fn new(db: Db) -> Result<Self> {
        Ok(Self {
            db,
            semaphore: Semaphore::new(10),
            workers: Default::default(),
        })
    }
}
