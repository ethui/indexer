CREATE TABLE chains (
  chain_id INTEGER NOT NULL,
  start_block INTEGER NOT NULL,
  last_known_block INTEGER NOT NULL,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (chain_id)
);
