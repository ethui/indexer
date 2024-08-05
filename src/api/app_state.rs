use std::sync::Arc;

use crate::{config::Config, db::Db, sync::RethProviderFactory};

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub config: Config,
    pub provider_factory: Arc<RethProviderFactory>,
}
