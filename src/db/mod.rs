use color_eyre::Result;
use diesel_async::{AsyncConnection, AsyncPgConnection};
use tokio::spawn;
use tokio::task::JoinHandle;

use crate::config::DbConfig;

pub struct Db {
    connection: AsyncPgConnection,
}

impl Db {
    pub async fn start(config: &DbConfig) -> Result<JoinHandle<Result<()>>> {
        let connection = AsyncPgConnection::establish(&config.url).await?;
        let db = Self { connection };

        let handle = spawn(async move { db.run().await });

        Ok(handle)
    }

    #[tracing::instrument(name = "db", skip(self))]
    async fn run(self) -> Result<()> {
        Ok(())
    }
}
