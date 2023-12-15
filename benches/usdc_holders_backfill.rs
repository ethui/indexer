mod utils;

use std::sync::Arc;

use color_eyre::Result;
use criterion::*;
use diesel::{
    sql_query,
    sql_types::{Array, Bytea, Integer},
    RunQueryDsl,
};
use iron_indexer::sync::RethProviderFactory;
use iron_indexer::{
    config::Config,
    db::{types::Address, Db},
    sync::{BackfillManager, StopStrategy},
};
use tokio::sync::mpsc;

use self::utils::one_time_setup;

/// truncates DB
/// seeds 1000 initial users
/// and creates a set of backfill jobs
fn setup(concurrency: usize, jobs: u64, job_size: u64) -> Result<Config> {
    let (mut config, mut conn) = utils::setup("benches/iron-indexer.toml")?;
    config.sync.backfill_concurrency = concurrency;

    let addresses: Vec<Address> =
        std::fs::read_to_string("benches/datasets/sepolia-usdc-holders.txt")?
            .lines()
            .take(1000)
            .map(|l| Address(l.parse().unwrap()))
            .collect();

    // create N non-overlapping jobs
    for i in 0..jobs {
        // the "+ 1" ensures each job is non-adjacent and does not reorg into a single large block
        let start_block = config.chain.start_block as i32 - i as i32 * (job_size as i32 * 2);
        sql_query(
            "INSERT INTO backfill_jobs (low, high, chain_id, addresses) VALUES ($1, $2, $3, $4)",
        )
        .bind::<Integer, _>(start_block - job_size as i32)
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

    let provider_factory = Arc::new(RethProviderFactory::new(&config, &chain)?);
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
fn backfill_1000jobsx1000blocks(c: &mut Criterion) {
    one_time_setup("benches/iron-indexer.toml").unwrap();

    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("backfill_1000jobsx1000blocks");
    group.sample_size(10);
    let jobs = 128;
    let job_size = 40;
    group.throughput(Throughput::Elements(jobs * job_size));

    for concurrency in [1, 16, 32, 64, 128].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            concurrency,
            |b, concurrency| {
                b.to_async(&rt).iter_batched(
                    || {
                        setup(*concurrency, jobs, job_size)
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

criterion_group!(benches, backfill_1000jobsx1000blocks);
criterion_main!(benches);
