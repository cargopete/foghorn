use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct GatewayConfig {
    pub api_key: String,
    #[serde(default = "default_gateway_url")]
    pub url: String,
    #[serde(default = "default_probe_count")]
    pub probe_count: u32,
}

fn default_gateway_url() -> String {
    "https://gateway.thegraph.com/api".to_string()
}

fn default_probe_count() -> u32 {
    8
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct FoghornConfig {
    pub database_url: String,
    pub network_subgraph_url: String,
    pub rpc_urls: HashMap<String, String>,
    pub reorg_threshold: u64,
    pub max_qps_per_indexer: f64,
    pub probe_interval_secs: u64,
    pub freshness_interval_secs: u64,
    pub api_port: u16,
    pub api_host: String,
    pub test_sets_dir: String,
    pub opted_in_indexers: Vec<IndexerConfig>,
    pub cors_origins: Vec<String>,
    pub gateway: Option<GatewayConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct IndexerConfig {
    pub address: String,
    pub url: String,
    pub auth_token: Option<String>,
    pub stake_grt: Option<String>,
}

impl Default for FoghornConfig {
    fn default() -> Self {
        Self {
            database_url: "postgres://dispatch:dispatch@drpc-postgres-1:5432/foghorn".to_string(),
            network_subgraph_url: String::new(),
            rpc_urls: HashMap::new(),
            reorg_threshold: 12,
            max_qps_per_indexer: 0.2,
            probe_interval_secs: 300,
            freshness_interval_secs: 30,
            api_port: 8080,
            api_host: "0.0.0.0".to_string(),
            test_sets_dir: "./test-sets".to_string(),
            opted_in_indexers: vec![],
            cors_origins: vec!["*".to_string()],
            gateway: None,
        }
    }
}

pub fn load_config() -> anyhow::Result<FoghornConfig> {
    let cfg = config::Config::builder()
        .add_source(config::File::with_name("config").required(false))
        .add_source(
            config::Environment::with_prefix("FOGHORN")
                .separator("__")
                .try_parsing(true),
        )
        .build()?;

    Ok(cfg.try_deserialize::<FoghornConfig>().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "Config deserialization failed, falling back to defaults");
        FoghornConfig::default()
    }))
}
