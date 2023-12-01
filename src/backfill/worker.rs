use std::collections::HashSet;
use std::sync::Arc;

use alloy_primitives::Address;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct Worker {
    inner: Arc<RwLock<Inner>>,
}

#[derive(Debug, Clone)]
pub struct Inner {
    pub addresses: HashSet<Address>,
    pub from_block: u64,
    pub to_block: u64,
}
