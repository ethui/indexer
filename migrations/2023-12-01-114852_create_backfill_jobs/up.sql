CREATE TABLE backfill_jobs (
  address BYTEA[] NOT NULL,
  chain_id INTEGER NOT NULL,
  from_block INTEGER NOT NULL,
  to_block INTEGER NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (chain_id, from_block, to_block),
  FOREIGN KEY (chain_id) REFERENCES chains (chain_id)
);
