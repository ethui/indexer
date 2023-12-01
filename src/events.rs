use alloy_primitives::Address;

#[derive(Debug, Clone)]
pub enum Event {
    AccountRegistered { address: Address },
}
