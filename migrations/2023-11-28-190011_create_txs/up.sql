CREATE TABLE txs (
  address BYTEA NOT NULL,
  chain_id INTEGER NOT NULL,
  hash BYTEA NOT NULL,
  block_number INTEGER NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (address, chain_id, hash),
  FOREIGN KEY (address, chain_id) REFERENCES accounts (address, chain_id)
);
