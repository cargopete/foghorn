-- Foghorn schema v1

CREATE TABLE IF NOT EXISTS probe (
    id                UUID PRIMARY KEY,
    deployment_id     TEXT NOT NULL,
    block_hash        TEXT NOT NULL,
    block_number      BIGINT NOT NULL,
    query_hash        TEXT NOT NULL,
    query_category    TEXT NOT NULL,        -- Q_byid | Q_agg | Q_freshness | Q_timetravel
    query_text        TEXT NOT NULL,
    dispatched_at     TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS probe_deployment_time ON probe (deployment_id, dispatched_at DESC);
CREATE INDEX IF NOT EXISTS probe_dispatched_at   ON probe (dispatched_at DESC);

CREATE TABLE IF NOT EXISTS observation (
    probe_id              UUID NOT NULL REFERENCES probe(id) ON DELETE CASCADE,
    indexer_address       TEXT NOT NULL,
    response_hash         TEXT,             -- NULL on error / no response
    latency_ms            INT,
    meta_block_number     BIGINT,
    meta_block_hash       TEXT,
    http_status           INT,
    error_class           TEXT,             -- NULL | 'network_error' | 'http_error' | 'graphql_error' | 'invalid_json'
    stake_weight          DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    PRIMARY KEY (probe_id, indexer_address)
);
CREATE INDEX IF NOT EXISTS obs_indexer        ON observation (indexer_address);
CREATE INDEX IF NOT EXISTS obs_response_hash  ON observation (response_hash);
CREATE INDEX IF NOT EXISTS obs_probe_id       ON observation (probe_id);

CREATE TABLE IF NOT EXISTS divergence (
    probe_id                  UUID PRIMARY KEY REFERENCES probe(id) ON DELETE CASCADE,
    cluster_count             INT NOT NULL,
    diff_patches              JSONB,        -- RFC 6902 patch array (capped at 256b values)
    largest_by_count_hash     TEXT NOT NULL,
    largest_by_count_size     INT NOT NULL,
    largest_by_stake_hash     TEXT NOT NULL,
    largest_by_stake_weight   DOUBLE PRECISION NOT NULL,
    created_at                TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS divergence_created_at ON divergence (created_at DESC);

CREATE TABLE IF NOT EXISTS freshness_sample (
    id                    BIGSERIAL PRIMARY KEY,
    indexer_address       TEXT NOT NULL,
    deployment_id         TEXT NOT NULL,
    sampled_at            TIMESTAMPTZ NOT NULL,
    meta_block_number     BIGINT NOT NULL,
    meta_block_hash       TEXT NOT NULL,
    chainhead_lag_blocks  INT NOT NULL
);
CREATE INDEX IF NOT EXISTS freshness_indexer_deployment
    ON freshness_sample (indexer_address, deployment_id, sampled_at DESC);

-- 90-day retention: raw probe/observation/freshness data
-- Rollup kept indefinitely.  Retention enforced by a cron job (not in schema).
