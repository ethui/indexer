mod utils;

use std::sync::Arc;

use color_eyre::Result;
use criterion::*;
use diesel::{
    sql_query,
    sql_types::{Array, Bytea, Integer},
    RunQueryDsl,
};
use iron_indexer::sync::RethProvider;
use iron_indexer::{
    config::Config,
    db::{types::Address, Db},
    sync::{BackfillManager, StopStrategy},
};
use tokio::sync::{mpsc, RwLock};

use self::utils::one_time_setup;

/// truncates DB
/// seeds 1000 initial users
/// and creates a set of backfill jobs
fn setup(total_blocks: usize, concurrency: usize, batch_size: i32) -> Result<Config> {
    let (mut config, mut conn) = utils::setup("benches/iron-indexer.toml")?;
    config.sync.backfill_concurrency = concurrency;

    let addresses: Vec<Address> =
        std::fs::read_to_string("benches/datasets/sepolia-usdc-holders.txt")?
            .lines()
            .take(1000)
            .map(|l| Address(l.parse().unwrap()))
            .collect();

    // create 100 non-overlapping jobs
    // let blocks_per_job: i32 = 1000;
    let batch_count = total_blocks as i32 / batch_size;
    for i in 0..batch_count {
        // the "+ 1" ensures each job is non-adjacent and does not reorg into a single large block
        let start_block = config.chain.start_block as i32 - i * (batch_size + 1);
        sql_query(
            "INSERT INTO backfill_jobs (low, high, chain_id, addresses) VALUES ($1, $2, $3, $4)",
        )
        .bind::<Integer, _>(start_block - batch_size)
        .bind::<Integer, _>(start_block)
        .bind::<Integer, _>(config.chain.chain_id)
        .bind::<Array<Bytea>, _>(&addresses[0..1])
        .execute(&mut conn)?;
    }

    Ok(config)
}

async fn run(config: Config) -> Result<()> {
    let (account_tx, _account_rx) = mpsc::unbounded_channel();
    let (job_tx, job_rx) = mpsc::unbounded_channel();
    let db = Db::connect(&config, account_tx, job_tx).await?;
    let chain = db.setup_chain(&config.chain).await?;

    let provider_factory = Arc::new(RwLock::new(RethProvider::new(&config, &chain)?));
    let backfill = BackfillManager::new(
        db.clone(),
        &config,
        provider_factory,
        job_rx,
        StopStrategy::OnFinish,
    );

    backfill.run().await?;

    Ok(())
}

/// Processes a total of 100k blocks in different configurations:
///   - from 1 to 10000 concurrent jobs
///   - job size varies from 1 block to 1000 blocks per job
fn backfill_100k_500job_size(c: &mut Criterion) {
    one_time_setup("benches/iron-indexer.toml").unwrap();

    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("backfill_100k_job500");
    group.sample_size(10);
    group.throughput(Throughput::Elements(100_000));

    let blocks = 10_000;
    let batch_size = 1000;
    for concurrency in [100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            concurrency,
            |b, concurrency| {
                b.to_async(&rt).iter_batched(
                    || {
                        setup(blocks, *concurrency, batch_size)
                            .unwrap_or_else(|e| panic!("{}", e.to_string()))
                    },
                    |config| async move { run(config).await },
                    BatchSize::LargeInput,
                )
            },
        );
    }

    group.finish();
}

criterion_group!(benches, backfill_100k_500job_size);
criterion_main!(benches);
