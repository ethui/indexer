mod utils;

use std::path::Path;

use color_eyre::Result;
use criterion::*;
use reth_db::open_db_read_only;
use reth_provider::{BlockReader, ProviderFactory, ReceiptProvider, TransactionsProvider};
use tokio::task;

async fn run_multiple_providers(blocks: u64, concurrency: usize) -> Result<()> {
    let spec = (*reth_primitives::SEPOLIA).clone();
    let path = Path::new("/mnt/data/eth/sepolia/reth/db");
    let db = open_db_read_only(&path, None)?;

    let factory: ProviderFactory<reth_db::DatabaseEnv> = ProviderFactory::new(db, spec.clone());

    let mut handles = Vec::new();
    let blocks_per_task = blocks as usize / concurrency;
    (0..concurrency).for_each(|i| {
        let provider = factory.provider().unwrap();
        let from = 4700000 + i * blocks_per_task * 2;
        let to = from + blocks_per_task;
        let handle = task::spawn(async move {
            for block in from..to {
                let indices = provider.block_body_indices(block as u64).unwrap().unwrap();
                for id in indices.first_tx_num..indices.first_tx_num + indices.tx_count {
                    let tx = provider.transaction_by_id_no_hash(id).unwrap();
                    let receipt = provider.receipt(id).unwrap();
                }
                //println!("finished {}", block);
            }
        });
        handles.push(handle);
    });

    for handle in handles {
        handle.await.unwrap();
    }

    Ok(())
}

/// Processes a total of 100k blocks in different configurations:
///   - from 1 to 10000 concurrent jobs
///   - job size varies from 1 block to 1000 blocks per job
fn provider_concurrency(c: &mut Criterion) {
    println!("PID: {}", std::process::id());
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("multiple_providers");
    group.sample_size(10);
    let blocks = 1000;
    group.throughput(Throughput::Elements(blocks));

    for concurrency in [1usize, 10, 100, 200, 400, 800].into_iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            &concurrency,
            |b, concurrency| {
                b.to_async(&rt)
                    .iter(|| async move { run_multiple_providers(blocks, *concurrency).await })
            },
        );
    }

    group.finish();
}

criterion_group!(benches, provider_concurrency);
criterion_main!(benches);
