use alloy_primitives::Address;
use diesel::prelude::*;

table! {
    accounts {
        id -> Integer,
        address -> String,
        chain_id -> Integer,
    }
}

#[derive(Queryable, Selectable, Debug)]
struct Accounts {
    id: i32,
    address: Address,
}
