// @generated automatically by Diesel CLI.

diesel::table! {
    accounts (address, chain_id) {
        address -> Bytea,
        chain_id -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    txs (address, chain_id, hash) {
        address -> Bytea,
        chain_id -> Int4,
        hash -> Bytea,
        block_number -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    accounts,
    txs,
);