# Iron Indexer

[reth-indexer]: https://github.com/joshstevens19/reth-indexer
[iron]: https://iron-wallet.xyz

A parallel Reth indexer.
It reads transaction history directly from [reth][reth] DB (directly from the filesystem, therefore skipping network & JSON-RPC overhead), and is able to index data from a dynamic dataset, by including target addresses midway through and backfilling all past data while regular sync continues.

Note: Credit to [reth-indexer][reth-indexer], which was the original implementation that served as a basis for this.

## Disclaimer

This is currently a prototype, and built to serve a yet-to-be-released feature of [Iron wallet][iron]. All development so far has been with that goal in mind. Don't expect a plug-and-play indexing solution for every use case (at least not right now)

## Why

Fetching on-chain data can be a painful process. A simple query such as "what is the transaction history for my wallet address" translates into a time-consuming walk of the entire chain.

In fact, transaction history is an even more illusive problem than it may seem:

- catching transactions where `from` / `to` match the given address is already an intensive task;
- arbitrary transactions may reference the address within the logs, even if they don't directly interact with it (e.g.: an ERC20 airdrop, where `from = contract_owner` and `to = contract` should arguably be considered too)
-

On top of this, in most indexers we are restricted to either:

- indexing a fixed set of topics, having to re-index from scratch if the set changes...
- or indexing all potential data upfront to avoid having to re-index

Instead, `iron-indexer` takes a different approach: new addresses can be added to the sync list at runtime, and self-optimizing backfill jobs are registered to backfill all data for each incoming address.

## Benchmarks

TODO

## Requirements

- A reth node running in the same node (requires access to the same filesystem)
- PostgreSQL
