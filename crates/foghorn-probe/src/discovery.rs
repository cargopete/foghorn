use anyhow::{anyhow, Result};
use foghorn_core::config::{FoghornConfig, IndexerConfig};
use tracing::info;

/// For v1: returns explicitly configured opted-in indexers.
/// Future: query network subgraph and filter by free-probe allow-list.
pub async fn get_opted_in_indexers(config: &FoghornConfig) -> Result<Vec<IndexerConfig>> {
    info!(
        count = config.opted_in_indexers.len(),
        "Using configured opted-in indexers"
    );
    Ok(config.opted_in_indexers.clone())
}

/// Get the block hash at (chainhead - reorg_threshold) for a given network RPC.
/// Returns (block_number, block_hash_hex).
pub async fn get_safe_block(rpc_url: &str, reorg_threshold: u64) -> Result<(u64, String)> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    // Step 1: get current block number
    let resp: serde_json::Value = client
        .post(rpc_url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        }))
        .send()
        .await?
        .json()
        .await?;

    let hex_num = resp["result"]
        .as_str()
        .ok_or_else(|| anyhow!("eth_blockNumber: no result"))?;
    let current_block = u64::from_str_radix(hex_num.trim_start_matches("0x"), 16)?;
    let safe_block = current_block.saturating_sub(reorg_threshold);

    // Step 2: get the block by number to retrieve its hash
    let resp: serde_json::Value = client
        .post(rpc_url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBlockByNumber",
            "params": [format!("0x{:x}", safe_block), false],
            "id": 2
        }))
        .send()
        .await?
        .json()
        .await?;

    let hash = resp["result"]["hash"]
        .as_str()
        .ok_or_else(|| anyhow!("eth_getBlockByNumber: no hash in result"))?
        .to_string();

    Ok((safe_block, hash))
}
