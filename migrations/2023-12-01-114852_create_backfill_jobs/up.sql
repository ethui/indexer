CREATE TABLE backfill_jobs (
  id SERIAL,
  address BYTEA NOT NULL,
  chain_id INTEGER NOT NULL,
  from_block INTEGER NOT NULL,
  to_block INTEGER NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  FOREIGN KEY (chain_id, address) REFERENCES accounts (chain_id, address)
);
