-- Maps allocation signing keys (ecrecover'd from graph-attestation header)
-- to real indexer addresses via the Graph Network subgraph.
CREATE TABLE IF NOT EXISTS allocation_map (
    allocation_key      TEXT PRIMARY KEY,   -- lowercase hex, = allocation ID
    indexer_address     TEXT,               -- real indexer address; NULL = not found
    indexer_url         TEXT,
    resolved_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS alloc_map_indexer ON allocation_map (indexer_address);
