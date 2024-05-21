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
    backfill_jobs (id) {
        id -> Int4,
        addresses -> Array<Bytea>,
        chain_id -> Int4,
        low -> Int4,
        high -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    chains (chain_id) {
        chain_id -> Int4,
        start_block -> Int4,
        last_known_block -> Int4,
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

diesel::joinable!(backfill_jobs -> chains (chain_id));

diesel::allow_tables_to_appear_in_same_query!(accounts, backfill_jobs, chains, txs,);
