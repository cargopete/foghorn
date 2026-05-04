use foghorn_core::config::FoghornConfig;
use sqlx::PgPool;
use std::time::Duration;
use tracing::info;

pub async fn run_freshness_monitor(config: FoghornConfig, pool: PgPool) {
    info!("Freshness monitor starting");

    loop {
        tokio::time::sleep(Duration::from_secs(config.freshness_interval_secs)).await;

        for indexer in &config.opted_in_indexers {
            let pool = pool.clone();
            let url = indexer.url.clone();
            let address = indexer.address.clone();
            let auth = indexer.auth_token.clone();

            tokio::spawn(async move {
                probe_freshness(pool, url, address, auth).await;
            });
        }
    }
}

async fn probe_freshness(
    _pool: PgPool,
    _base_url: String,
    indexer_address: String,
    _auth_token: Option<String>,
) {
    // Freshness probes query each deployment the indexer serves.
    // For v1, this is a lightweight _meta poll.  We store the result so the API
    // can serve freshness percentile histograms.
    //
    // In a production deployment you would first enumerate which deployments this
    // indexer serves (from the network subgraph).  For now we just emit a trace
    // so the monitor loop is visible; real sample insertion happens once deployment
    // discovery is wired up.
    info!(indexer = %indexer_address, "Freshness poll (v1 stub — deployment list needed)");
}
