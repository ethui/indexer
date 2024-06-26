# ethui Indexer

[reth]: https://paradigmxyz.github.io/reth/intro.html
[reth-indexer]: https://github.com/joshstevens19/reth-indexer
[ethui]: https://ethui.dev
[miguel]: https://twitter.com/naps62
[cuckoo]: https://en.wikipedia.org/wiki/Cuckoo_filter

A parallel Reth indexer.

Reads transaction history from [reth][reth]'s DB (direct from filesystem, skipping network & JSON-RPC overhead). It's able to index from a dynamic set of addresses, which can grow at runtime, by spawning parallel self-optimizing backfill jobs.

**Note**: Kudos to [reth-indexer][reth-indexer], which was the original implementation that served as a basis for this.

## Disclaimer

This is currently a prototype, and built to serve a yet-to-be-released feature of [ethui][ethui]. All development so far has been with that goal in mind. Don't expect a plug-and-play indexing solution for every use case (at least not right now)

## How to use

🚧 TODO 🚧

For now, check `ethui-indexer.toml`, which should help you get started. Feel free to contact [me][miguel] or open issues for any questions.

## Why

Fetching on-chain data can be a painful process. A simple query such as _"what is the transaction history for my wallet address?"_ translates into a time-consuming walk of the entire chain.
It's also not enough to sync the `from` and `to` fields of every transaction (which would already be costly). Relevant transactions for a wallet are also based on the emitted topics, such as an ERC20 transfers.

On top of this, most indexers require a predetermined set of topics to index, and any changes require a new full walk of the chain.

Instead, `ethui-indexer` takes a different approach: new addresses can be added to the sync list at runtime, and self-optimizing backfill jobs are registered to backfill all data for each incoming address.

## How

### Forward & Backfill workers

Let's illustrate this with an example: Say we're currently indexing only `alice`'s address. A regular syncing process is running, waiting for new blocks to process.

After block 10, `bob`'s address is added to the set. From block 11 onwards, both `alice` and `bob` will be matched. But we missed blocks 1 through 10 for `bob`. At this point we register a new backill job for the missing data.

We're now at this state:

| job             | account set    | block range     |
| --------------- | -------------- | --------------- |
| **Forward**     | `[alice, bob]` | waiting for #11 |
| **Backfill #1** | `[bob]`        | `[1, 10]`       |

The new job starts immediately, in reverse order.

A few moments later, `carol`'s address joins too. By now both existing jobs have advanced a bit:

| job             | account set    | block range     | notes                                     |
| --------------- | -------------- | --------------- | ----------------------------------------- |
| **Forward**     | `[alice, bob]` | waiting for #16 |                                           |
| **Backfill #1** | `[bob]`        | `[1, 5]`        | We've synced from 10 to 6 in the meantime |
| **Backfill #2** | `[carol]`      | `[1, 15]`       |                                           |

The naive approach would be to the new job and run all 3 concurrently.
This has one drawback thought: both backfill jobs will fetch redundant blocks (1 through 5).

Instead of starting right away, we run a [reorganization step](https://github.com/ethui/indexer/blob/main/src/rearrange.rs):

| job             | account set    | block range     | notes                                  |
| --------------- | -------------- | --------------- | -------------------------------------- |
| **Forward**     | `[alice, bob]` | waiting for #16 |                                        |
| **Backfill #3** | `[bob,carol]`  | `[1, 5]`        | The overlapping range in one job...    |
| **Backfill #4** | `[carol]`      | `[6, 15]`       | ...And carol's unique range in another |

This ensures we are never attempting to fetch the same block twice, therefore optimizing IO as much as possible.

### Cuckoo filters

We make use of [Cuckoo filters][cuckoo] for efficiently filtering data inclusion. This is similar to how Bloom filters work, with additional benefits such as ability to remove items, and lower space overhead. The particular [implementation being used](https://docs.rs/scalable_cuckoo_filter/0.2.3/scalable_cuckoo_filter/index.html) also supports automatic scaling.

## Future Work

### To be done next

- [ ] Finish the API
- [ ] Add EIP-712 based authentication
- [ ] Document this a bit better
- [ ] Benchmark on a real mainnet node

### Future optimizations

A few potential optimizations are still yet-to-be-done, but should help improve throughput even further:

- [ ] Split workers into producer/consumers. Currently workers alternate between fetching a block and processing. Instead, which is not optimal for IO. (question: is this worth it? or can we just saturate read capacity by setting up more workers?);
- [ ] Work-stealing. If we have a single backfill job walking N blocks, we can split it into Y jobs of N/Y blocks each. This can be done directly in the reorganization step.

## Benchmarks

🚧 TODO 🚧

## Requirements

- A reth node running in the same node (requires access to the same filesystem)
- PostgreSQL

## License

[MIT](./LICENSE) License
