use foghorn_core::{
    config::load_config,
    db::{create_pool, run_migrations},
};
use tracing::info;

mod cluster;
mod discovery;
mod executor;
mod freshness;
mod resolver;
mod scheduler;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("foghorn_probe=info".parse()?)
                .add_directive("reqwest=warn".parse()?),
        )
        .init();

    info!("Foghorn probe service starting");

    let config = load_config()?;
    let pool = create_pool(&config.database_url).await?;
    run_migrations(&pool).await?;

    info!("Database connected and migrations applied");

    // Spawn freshness monitor alongside the probe scheduler
    let freshness_pool = pool.clone();
    let freshness_config = config.clone();
    tokio::spawn(async move {
        freshness::run_freshness_monitor(freshness_config, freshness_pool).await;
    });

    scheduler::run_probe_scheduler(config, pool).await?;

    Ok(())
}
