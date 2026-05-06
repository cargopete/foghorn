use foghorn_core::normalize::strip_volatile;
use serde_json::Value;
use std::collections::HashMap;

pub struct ClusterInput {
    pub indexer_address: String,
    pub response_hash: Option<String>,
    pub raw_response: Option<String>,
    pub stake_weight: f64,
}

pub struct ClusterResult {
    pub cluster_count: i32,
    pub diff_patches: Value,
    pub largest_by_count_hash: String,
    pub largest_by_count_size: i32,
    pub largest_by_stake_hash: String,
    pub largest_by_stake_weight: f64,
    pub is_divergent: bool,
}

pub fn compute_clusters(inputs: &[ClusterInput]) -> ClusterResult {
    // Group successful observations by response hash
    let mut hash_members: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    let mut hash_to_raw: HashMap<String, String> = HashMap::new();

    for input in inputs {
        let Some(ref hash) = input.response_hash else { continue };
        hash_members
            .entry(hash.clone())
            .or_default()
            .push((input.indexer_address.clone(), input.stake_weight));
        if let Some(ref raw) = input.raw_response {
            hash_to_raw.entry(hash.clone()).or_insert_with(|| raw.clone());
        }
    }

    if hash_members.is_empty() {
        return ClusterResult {
            cluster_count: 0,
            diff_patches: Value::Array(vec![]),
            largest_by_count_hash: String::new(),
            largest_by_count_size: 0,
            largest_by_stake_hash: String::new(),
            largest_by_stake_weight: 0.0,
            is_divergent: false,
        };
    }

    // Build sorted cluster list
    struct Cluster {
        hash: String,
        members: Vec<String>,
        stake_weight: f64,
    }

    let mut clusters: Vec<Cluster> = hash_members
        .into_iter()
        .map(|(hash, members)| {
            let stake_weight: f64 = members.iter().map(|(_, w)| w).sum();
            let addrs = members.into_iter().map(|(a, _)| a).collect();
            Cluster { hash, members: addrs, stake_weight }
        })
        .collect();

    let is_divergent = clusters.len() > 1;

    // Largest by count
    let by_count_idx = clusters
        .iter()
        .enumerate()
        .max_by_key(|(_, c)| c.members.len())
        .map(|(i, _)| i)
        .unwrap_or(0);

    // Largest by stake
    let by_stake_idx = clusters
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.stake_weight.partial_cmp(&b.stake_weight).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0);

    // Diff: largest-by-count vs next largest
    let diff_patches = if is_divergent {
        let second_idx = clusters
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != by_count_idx)
            .max_by_key(|(_, c)| c.members.len())
            .map(|(i, _)| i)
            .unwrap_or(if by_count_idx == 0 { 1 } else { 0 });

        let hash_a = &clusters[by_count_idx].hash;
        let hash_b = &clusters[second_idx].hash;

        match (hash_to_raw.get(hash_a), hash_to_raw.get(hash_b)) {
            (Some(a), Some(b)) => compute_diff(a, b),
            _ => Value::Array(vec![]),
        }
    } else {
        Value::Array(vec![])
    };

    let largest_by_count = &clusters[by_count_idx];
    let largest_by_stake = &clusters[by_stake_idx];

    ClusterResult {
        cluster_count: clusters.len() as i32,
        diff_patches,
        largest_by_count_hash: largest_by_count.hash.clone(),
        largest_by_count_size: largest_by_count.members.len() as i32,
        largest_by_stake_hash: largest_by_stake.hash.clone(),
        largest_by_stake_weight: largest_by_stake.stake_weight,
        is_divergent,
    }
}

fn compute_diff(raw_a: &str, raw_b: &str) -> Value {
    let Ok(a) = serde_json::from_str::<Value>(raw_a) else { return Value::Array(vec![]) };
    let Ok(b) = serde_json::from_str::<Value>(raw_b) else { return Value::Array(vec![]) };
    // Strip the same volatile fields as the hash pipeline so the diff
    // reflects genuine data differences, not block-height noise.
    let a = strip_volatile(a);
    let b = strip_volatile(b);
    let patch = json_patch::diff(&a, &b);
    serde_json::to_value(patch).unwrap_or(Value::Array(vec![]))
}
