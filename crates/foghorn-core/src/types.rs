use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Probe {
    pub id: Uuid,
    pub deployment_id: String,
    pub block_hash: String,
    pub block_number: i64,
    pub query_hash: String,
    pub query_category: String,
    pub query_text: String,
    pub dispatched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub probe_id: Uuid,
    pub indexer_address: String,
    pub response_hash: Option<String>,
    pub latency_ms: Option<i32>,
    pub meta_block_number: Option<i64>,
    pub meta_block_hash: Option<String>,
    pub http_status: Option<i32>,
    pub error_class: Option<String>,
    pub stake_weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Divergence {
    pub probe_id: Uuid,
    pub cluster_count: i32,
    pub diff_patches: serde_json::Value,
    pub largest_by_count_hash: String,
    pub largest_by_count_size: i32,
    pub largest_by_stake_hash: String,
    pub largest_by_stake_weight: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessSample {
    pub id: i64,
    pub indexer_address: String,
    pub deployment_id: String,
    pub sampled_at: DateTime<Utc>,
    pub meta_block_number: i64,
    pub meta_block_hash: String,
    pub chainhead_lag_blocks: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterSummary {
    pub hash: String,
    pub member_count: usize,
    pub stake_weight: f64,
    pub members: Vec<String>,
    pub is_largest_by_count: bool,
    pub is_largest_by_stake: bool,
}

// Test set types — loaded from YAML
#[derive(Debug, Clone, Deserialize)]
pub struct TestSet {
    pub deployment: TestSetDeployment,
    pub queries: Vec<TestQuery>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestSetDeployment {
    pub id: String,
    pub ipfs_hash: String,
    pub network: String,
    pub description: String,
    /// Subgraph ID used with The Graph gateway (base58 format, e.g. "J55C6V...")
    #[serde(default)]
    pub gateway_subgraph_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestQuery {
    pub category: String,
    pub template: String,
    #[serde(default)]
    pub entity_ids: Vec<String>,
}
