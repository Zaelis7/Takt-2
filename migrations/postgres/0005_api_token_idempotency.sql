-- IAM-027; Requirements: PRD-API-003, PRD-IAM-001, PRD-IAM-005,
-- PRD-DATA-001, PRD-DATA-002, PRD-DATA-004, PRD-NFR-002, PRD-NFR-005.
CREATE TABLE api_token_idempotency (
    actor_type VARCHAR(16) NOT NULL CHECK (actor_type IN ('system', 'local_cli')),
    actor_id UUID NOT NULL,
    method VARCHAR(8) NOT NULL CHECK (method IN ('POST', 'PATCH', 'DELETE')),
    path VARCHAR(512) NOT NULL CHECK (length(path) >= 1),
    idempotency_key VARCHAR(128) NOT NULL CHECK (length(idempotency_key) BETWEEN 8 AND 128),
    request_hash BYTEA NOT NULL CHECK (octet_length(request_hash) = 32),
    api_token_id UUID NULL REFERENCES api_tokens(id) ON DELETE CASCADE,
    result_version BIGINT NULL CHECK (result_version >= 1),
    replay_key_version INTEGER NULL CHECK (replay_key_version >= 1),
    replay_nonce BYTEA NULL CHECK (octet_length(replay_nonce) = 12),
    replay_ciphertext BYTEA NULL CHECK (octet_length(replay_ciphertext) BETWEEN 17 AND 65552),
    created_at TIMESTAMPTZ(6) NOT NULL,
    expires_at TIMESTAMPTZ(6) NOT NULL,
    PRIMARY KEY (actor_type, actor_id, method, path, idempotency_key),
    CHECK (expires_at = created_at + INTERVAL '24 hours'),
    CHECK ((api_token_id IS NULL) = (result_version IS NULL)),
    CHECK (
        (method = 'POST' AND (
            (api_token_id IS NULL AND result_version IS NULL
                AND replay_key_version IS NULL AND replay_nonce IS NULL AND replay_ciphertext IS NULL)
            OR (api_token_id IS NOT NULL AND result_version IS NOT NULL
                AND replay_key_version IS NOT NULL AND replay_nonce IS NOT NULL AND replay_ciphertext IS NOT NULL)
        ))
        OR (method IN ('PATCH', 'DELETE') AND replay_key_version IS NULL AND replay_nonce IS NULL AND replay_ciphertext IS NULL)
    )
);
CREATE INDEX api_token_idempotency_expiry_idx ON api_token_idempotency (expires_at);
