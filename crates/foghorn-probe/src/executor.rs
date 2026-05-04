use foghorn_core::normalize::normalize_and_hash;
use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use sha3::{Digest, Keccak256};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

// Pre-computed EIP-712 constants for The Graph attestation scheme.
// Domain: name="Graph Protocol", version="0", chainId=42161 (Arbitrum),
//         verifyingContract=DisputeManager (0x2fe023a575449acb698648ed21276293fa176f96)
const RECEIPT_TYPEHASH: [u8; 32] =
    hex_bytes("32dd026408194a0d7e54cc66a2ab6c856efc55cfcd4dd258fde5b1a55222baa6");
const DOMAIN_SEPARATOR: [u8; 32] =
    hex_bytes("f7275085102bb2226cb6bb073e53c88272fb69f73b27bab0ac1e25eb0935dfc0");

const fn hex_bytes<const N: usize>(s: &str) -> [u8; N] {
    let s = s.as_bytes();
    let mut out = [0u8; N];
    let mut i = 0;
    while i < N {
        let hi = hex_nibble(s[i * 2]);
        let lo = hex_nibble(s[i * 2 + 1]);
        out[i] = (hi << 4) | lo;
        i += 1;
    }
    out
}

const fn hex_nibble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

pub struct ProbeRequest {
    pub indexer_address: String,
    pub indexer_url: String,
    pub deployment_ipfs_hash: String,
    pub query: String,
    pub block_hash: String,
    pub auth_token: Option<String>,
    pub stake_weight: f64,
}

pub struct GatewayProbeRequest {
    pub gateway_url: String,
    pub api_key: String,
    pub subgraph_id: String,
    pub _deployment_id: String,
    pub query: String,
    pub block_hash: String,
}

pub struct RawObservation {
    pub indexer_address: String,
    pub response_hash: Option<String>,
    pub raw_response: Option<String>,
    pub latency_ms: i32,
    pub meta_block_number: Option<i64>,
    pub meta_block_hash: Option<String>,
    pub http_status: Option<i32>,
    pub error_class: Option<String>,
    pub stake_weight: f64,
}

/// Execute a probe via The Graph gateway. The response hash is taken from
/// the `graph-attestation` header (`responseCID`), and the indexer address
/// is recovered from the EIP-712 signature in that header.
pub async fn execute_gateway_probe(req: GatewayProbeRequest) -> RawObservation {
    let url = format!(
        "{}/{}/subgraphs/id/{}",
        req.gateway_url.trim_end_matches('/'),
        req.api_key,
        req.subgraph_id,
    );

    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    else {
        return gateway_error_observation("client_build");
    };

    let body = serde_json::json!({
        "query": req.query,
        "variables": { "block": { "hash": req.block_hash } }
    });

    let start = Instant::now();

    let resp = match client.post(&url).json(&body).send().await {
        Err(e) => {
            warn!(error = %e, "Gateway probe network error");
            return gateway_error_observation("network_error");
        }
        Ok(r) => r,
    };

    let http_status = resp.status().as_u16() as i32;
    let latency_ms = start.elapsed().as_millis() as i32;
    let attestation_header = resp
        .headers()
        .get("graph-attestation")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    if !resp.status().is_success() {
        return RawObservation {
            indexer_address: "gateway-error".to_string(),
            response_hash: None,
            raw_response: None,
            latency_ms,
            meta_block_number: None,
            meta_block_hash: None,
            http_status: Some(http_status),
            error_class: Some("http_error".to_string()),
            stake_weight: 1.0,
        };
    }

    let body_text = match resp.text().await {
        Err(e) => {
            warn!(error = %e, "Gateway probe body read error");
            return gateway_error_observation("body_error");
        }
        Ok(t) => t,
    };

    let parsed: Option<serde_json::Value> = serde_json::from_str(&body_text).ok();

    let error_class = parsed.as_ref().and_then(|v| {
        if v.get("errors").is_some() {
            Some("graphql_error".to_string())
        } else {
            None
        }
    });

    let (meta_block_number, meta_block_hash) = parsed
        .as_ref()
        .map(extract_meta)
        .unwrap_or((None, None));

    // Recover indexer address from attestation; use JCS hash for content clustering.
    let indexer_address = if let Some(attest_str) = &attestation_header {
        parse_attestation_address(attest_str)
    } else {
        "gateway-no-attestation".to_string()
    };
    let response_hash = if error_class.is_none() {
        normalize_and_hash(&body_text).ok()
    } else {
        None
    };

    debug!(
        indexer = %indexer_address,
        hash = ?response_hash,
        latency_ms,
        "Gateway probe complete"
    );

    RawObservation {
        indexer_address,
        response_hash,
        raw_response: Some(body_text),
        latency_ms,
        meta_block_number,
        meta_block_hash,
        http_status: Some(http_status),
        error_class,
        stake_weight: 1.0,
    }
}

/// Parse the `graph-attestation` JSON header and recover the signing address.
/// The address is the allocation-specific key (unique per indexer allocation).
fn parse_attestation_address(header: &str) -> String {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(header) else {
        return "gateway-bad-attestation".to_string();
    };
    try_recover_signer(&v).unwrap_or_else(|| "gateway-unresolved".to_string())
}

fn try_recover_signer(v: &serde_json::Value) -> Option<String> {
    let parse_hex32 = |key: &str| -> Option<[u8; 32]> {
        let s = v[key].as_str()?.trim_start_matches("0x");
        if s.len() != 64 {
            return None;
        }
        let bytes = hex::decode(s).ok()?;
        bytes.try_into().ok()
    };

    let request_cid = parse_hex32("requestCID")?;
    let response_cid = parse_hex32("responseCID")?;
    let deployment_id = parse_hex32("subgraphDeploymentID")?;
    let r_bytes = parse_hex32("r")?;
    let s_bytes = parse_hex32("s")?;
    let v_val = v["v"].as_u64()? as u8;

    // Build EIP-712 hash: keccak256(0x1901 || DOMAIN_SEPARATOR || receipt_hash)
    // receipt_hash = keccak256(abi.encode(RECEIPT_TYPEHASH, requestCID, responseCID, deploymentID))
    let mut receipt_encoded = [0u8; 128];
    receipt_encoded[0..32].copy_from_slice(&RECEIPT_TYPEHASH);
    receipt_encoded[32..64].copy_from_slice(&request_cid);
    receipt_encoded[64..96].copy_from_slice(&response_cid);
    receipt_encoded[96..128].copy_from_slice(&deployment_id);
    let receipt_hash = keccak256(&receipt_encoded);

    let mut preimage = [0u8; 66];
    preimage[0] = 0x19;
    preimage[1] = 0x01;
    preimage[2..34].copy_from_slice(&DOMAIN_SEPARATOR);
    preimage[34..66].copy_from_slice(&receipt_hash);
    let msg_hash = keccak256(&preimage);

    let sig = Signature::from_scalars(r_bytes, s_bytes).ok()?;
    let recovery_id = RecoveryId::try_from(v_val).ok()?;
    let vk = VerifyingKey::recover_from_prehash(&msg_hash, &sig, recovery_id).ok()?;

    let uncompressed = vk.to_encoded_point(false);
    let pubkey_bytes = &uncompressed.as_bytes()[1..]; // skip 0x04
    let hash = keccak256(pubkey_bytes);
    Some(format!("0x{}", hex::encode(&hash[12..])))
}

fn keccak256(input: &[u8]) -> [u8; 32] {
    let mut h = Keccak256::new();
    h.update(input);
    h.finalize().into()
}

// ── Direct indexer probe (legacy free-query mode) ────────────────────────────

pub async fn execute_probe(req: ProbeRequest) -> RawObservation {
    let url = format!(
        "{}/subgraphs/id/{}",
        req.indexer_url.trim_end_matches('/'),
        req.deployment_ipfs_hash
    );

    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    else {
        return error_observation(req.indexer_address, req.stake_weight, None, "client_build");
    };

    let mut builder = client.post(&url);
    if let Some(ref token) = req.auth_token {
        builder = builder.bearer_auth(token);
    }

    let body = serde_json::json!({
        "query": req.query,
        "variables": { "block": { "hash": req.block_hash } }
    });

    let start = Instant::now();

    let resp = match builder.json(&body).send().await {
        Err(e) => {
            warn!(indexer = %req.indexer_address, error = %e, "Probe network error");
            return error_observation(
                req.indexer_address,
                req.stake_weight,
                Some(start.elapsed().as_millis() as i32),
                "network_error",
            );
        }
        Ok(r) => r,
    };

    let http_status = resp.status().as_u16() as i32;
    let latency_ms = start.elapsed().as_millis() as i32;

    if !resp.status().is_success() {
        warn!(indexer = %req.indexer_address, status = http_status, "Probe HTTP error");
        return RawObservation {
            indexer_address: req.indexer_address,
            response_hash: None,
            raw_response: None,
            latency_ms,
            meta_block_number: None,
            meta_block_hash: None,
            http_status: Some(http_status),
            error_class: Some("http_error".to_string()),
            stake_weight: req.stake_weight,
        };
    }

    let body_text = match resp.text().await {
        Err(e) => {
            warn!(indexer = %req.indexer_address, error = %e, "Probe body read error");
            return RawObservation {
                indexer_address: req.indexer_address,
                response_hash: None,
                raw_response: None,
                latency_ms,
                meta_block_number: None,
                meta_block_hash: None,
                http_status: Some(http_status),
                error_class: Some("body_error".to_string()),
                stake_weight: req.stake_weight,
            };
        }
        Ok(t) => t,
    };

    let parsed: Option<serde_json::Value> = serde_json::from_str(&body_text).ok();

    let error_class = parsed.as_ref().and_then(|v| {
        if v.get("errors").is_some() {
            Some("graphql_error".to_string())
        } else {
            None
        }
    }).or_else(|| {
        if parsed.is_none() { Some("invalid_json".to_string()) } else { None }
    });

    let (meta_block_number, meta_block_hash) = parsed
        .as_ref()
        .map(extract_meta)
        .unwrap_or((None, None));

    let response_hash = if error_class.is_none() {
        normalize_and_hash(&body_text).ok()
    } else {
        None
    };

    debug!(
        indexer = %req.indexer_address,
        hash = ?response_hash,
        latency_ms,
        "Probe complete"
    );

    RawObservation {
        indexer_address: req.indexer_address,
        response_hash,
        raw_response: Some(body_text),
        latency_ms,
        meta_block_number,
        meta_block_hash,
        http_status: Some(http_status),
        error_class,
        stake_weight: req.stake_weight,
    }
}

fn gateway_error_observation(class: &str) -> RawObservation {
    RawObservation {
        indexer_address: "gateway-error".to_string(),
        response_hash: None,
        raw_response: None,
        latency_ms: 0,
        meta_block_number: None,
        meta_block_hash: None,
        http_status: None,
        error_class: Some(class.to_string()),
        stake_weight: 1.0,
    }
}

fn error_observation(addr: String, stake_weight: f64, latency_ms: Option<i32>, class: &str) -> RawObservation {
    RawObservation {
        indexer_address: addr,
        response_hash: None,
        raw_response: None,
        latency_ms: latency_ms.unwrap_or(0),
        meta_block_number: None,
        meta_block_hash: None,
        http_status: None,
        error_class: Some(class.to_string()),
        stake_weight,
    }
}

fn extract_meta(value: &serde_json::Value) -> (Option<i64>, Option<String>) {
    let block = value.pointer("/data/_meta/block");
    match block {
        Some(b) => (
            b["number"].as_i64(),
            b["hash"].as_str().map(str::to_string),
        ),
        None => (None, None),
    }
}
