use crate::{config::Config, db::Db};

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub config: Config,
}
