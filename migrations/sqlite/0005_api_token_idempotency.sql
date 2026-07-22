-- IAM-027; Requirements: PRD-API-003, PRD-IAM-001, PRD-IAM-005,
-- PRD-DATA-001, PRD-DATA-002, PRD-DATA-004, PRD-NFR-002, PRD-NFR-005.
CREATE TABLE api_token_idempotency (
    actor_type TEXT NOT NULL CHECK (actor_type IN ('system', 'local_cli')),
    actor_id TEXT NOT NULL CHECK (length(actor_id) = 36 AND substr(actor_id, 15, 1) = '7'),
    method TEXT NOT NULL CHECK (method IN ('POST', 'PATCH', 'DELETE')),
    path TEXT NOT NULL CHECK (length(path) BETWEEN 1 AND 512),
    idempotency_key TEXT NOT NULL CHECK (length(idempotency_key) BETWEEN 8 AND 128),
    request_hash BLOB NOT NULL CHECK (length(request_hash) = 32),
    api_token_id TEXT NULL REFERENCES api_tokens(id) ON DELETE CASCADE,
    result_version INTEGER NULL CHECK (result_version >= 1),
    replay_key_version INTEGER NULL CHECK (replay_key_version >= 1),
    replay_nonce BLOB NULL CHECK (length(replay_nonce) = 12),
    replay_ciphertext BLOB NULL CHECK (length(replay_ciphertext) BETWEEN 17 AND 65552),
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL CHECK (expires_at = created_at + 86400000000),
    PRIMARY KEY (actor_type, actor_id, method, path, idempotency_key),
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
) STRICT;
CREATE INDEX api_token_idempotency_expiry_idx ON api_token_idempotency (expires_at);
