mod utils;

use color_eyre::Result;
use criterion::*;
use diesel::sql_types::{Array, Bytea, Integer};
use diesel::{sql_query, RunQueryDsl};
use iron_indexer::{
    config::Config,
    db::{types::Address, Db},
    sync::BackfillManager,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// truncates DB
/// seeds 1000 initial users
/// and creates a set of backfill jobs
fn setup() -> Result<Config> {
    let (config, mut conn) = utils::setup("benches/iron-indexer.toml")?;

    let addresses: Vec<Address> =
        std::fs::read_to_string("benches/datasets/sepolia-usdc-holders.txt")?
            .lines()
            .take(1000)
            .map(|l| Address(l.parse().unwrap()))
            .collect();

    for address in addresses.iter() {
        sql_query("INSERT INTO accounts (address, chain_id) VALUES ($1, $2)")
            .bind::<Bytea, _>(address)
            .bind::<Integer, _>(config.chain.chain_id)
            .execute(&mut conn)
            .unwrap();
    }

    sql_query("INSERT INTO chains (chain_id, start_block, last_known_block) VALUES ($1, $2, $3)")
        .bind::<Integer, _>(config.chain.chain_id)
        .bind::<Integer, _>(0)
        .bind::<Integer, _>(config.chain.start_block as i32)
        .execute(&mut conn)?;

    // create 100 non-overlapping jobs
    let blocks_per_job: i32 = 1000;
    for i in 0..3 {
        // let start_block = config.chain.start_block as i32 - i * blocks_per_job;
        let start_block = 10000 - i * blocks_per_job;
        sql_query(
            "INSERT INTO backfill_jobs (low, high, chain_id, addresses) VALUES ($1, $2, $3, $4)",
        )
        .bind::<Integer, _>(start_block - blocks_per_job)
        .bind::<Integer, _>(start_block)
        .bind::<Integer, _>(config.chain.chain_id)
        .bind::<Array<Bytea>, _>(&addresses[0..1])
        .execute(&mut conn)?;
    }

    Ok(config)
}

async fn run(config: Config) -> Result<()> {
    dbg!("running");
    let (account_tx, _account_rx) = mpsc::unbounded_channel();
    let (job_tx, job_rx) = mpsc::unbounded_channel();
    let db = Db::connect(&config, account_tx, job_tx).await?;

    let token = CancellationToken::new();
    let mut backfill = BackfillManager::new(db.clone(), &config, job_rx, token.clone());

    #[cfg(feature = "bench")]
    backfill.close_when_empty();

    backfill.run().await?;

    Ok(())
}

fn usdc_holders_backfill(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("usdc_holders");
    group.sample_size(10);

    group.bench_function("usdc_holders_backfill", move |b| {
        b.to_async(&rt).iter_batched(
            || setup().unwrap_or_else(|e| panic!("{}", e.to_string())),
            |config| async move { run(config).await },
            BatchSize::LargeInput,
        )
    });
}

criterion_group!(benches, usdc_holders_backfill);
criterion_main!(benches);
