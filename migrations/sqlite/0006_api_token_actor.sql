-- IAM-038; Requirements: PRD-API-003, PRD-IAM-001, PRD-IAM-004,
-- PRD-IAM-005, PRD-DATA-001, PRD-DATA-002, PRD-DATA-004,
-- PRD-NFR-002, PRD-NFR-005.
DROP TRIGGER audit_events_reject_update;
DROP TRIGGER audit_events_reject_delete;
ALTER TABLE audit_events RENAME TO audit_events_before_api_token_actor;

CREATE TABLE audit_events (
    id TEXT PRIMARY KEY CHECK (length(id) = 36 AND substr(id, 15, 1) = '7'),
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    project_id TEXT NULL,
    actor_type TEXT NOT NULL CHECK (actor_type IN ('system', 'local_cli', 'api_token')),
    actor_id TEXT NULL REFERENCES users(id) ON DELETE RESTRICT,
    api_token_actor_id TEXT NULL REFERENCES api_tokens(id) ON DELETE RESTRICT,
    action TEXT NOT NULL CHECK (length(action) <= 120),
    resource_type TEXT NOT NULL CHECK (length(resource_type) <= 64),
    resource_id TEXT NOT NULL CHECK (length(resource_id) = 36 AND substr(resource_id, 15, 1) = '7'),
    request_id TEXT NOT NULL CHECK (length(request_id) = 36 AND substr(request_id, 15, 1) = '7'),
    source_ip_hash TEXT NULL,
    before_hash TEXT NULL,
    after_hash TEXT NULL,
    metadata TEXT NOT NULL CHECK (json_valid(metadata)),
    occurred_at INTEGER NOT NULL,
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE RESTRICT,
    CHECK (
        (actor_type = 'api_token' AND actor_id IS NULL AND api_token_actor_id IS NOT NULL)
        OR (actor_type IN ('system', 'local_cli') AND api_token_actor_id IS NULL)
    )
) STRICT;

INSERT INTO audit_events (
    id, organization_id, project_id, actor_type, actor_id, api_token_actor_id,
    action, resource_type, resource_id, request_id, source_ip_hash,
    before_hash, after_hash, metadata, occurred_at
)
SELECT
    id, organization_id, project_id, actor_type, actor_id, NULL,
    action, resource_type, resource_id, request_id, source_ip_hash,
    before_hash, after_hash, metadata, occurred_at
FROM audit_events_before_api_token_actor;
DROP TABLE audit_events_before_api_token_actor;

CREATE INDEX audit_events_organization_occurred_idx
    ON audit_events (organization_id, occurred_at DESC);
CREATE INDEX audit_events_project_occurred_idx
    ON audit_events (project_id, occurred_at DESC);
CREATE INDEX audit_events_request_id_idx ON audit_events (request_id);
CREATE INDEX audit_events_api_token_actor_idx
    ON audit_events (api_token_actor_id)
    WHERE api_token_actor_id IS NOT NULL;

CREATE TRIGGER audit_events_reject_update
BEFORE UPDATE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit_events are append-only');
END;

CREATE TRIGGER audit_events_reject_delete
BEFORE DELETE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit_events are append-only');
END;

ALTER TABLE api_token_idempotency
    RENAME TO api_token_idempotency_before_api_token_actor;
CREATE TABLE api_token_idempotency (
    actor_type TEXT NOT NULL CHECK (actor_type IN ('system', 'local_cli', 'api_token')),
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
INSERT INTO api_token_idempotency (
    actor_type, actor_id, method, path, idempotency_key, request_hash,
    api_token_id, result_version, replay_key_version, replay_nonce,
    replay_ciphertext, created_at, expires_at
)
SELECT
    actor_type, actor_id, method, path, idempotency_key, request_hash,
    api_token_id, result_version, replay_key_version, replay_nonce,
    replay_ciphertext, created_at, expires_at
FROM api_token_idempotency_before_api_token_actor;
DROP TABLE api_token_idempotency_before_api_token_actor;
CREATE INDEX api_token_idempotency_expiry_idx
    ON api_token_idempotency (expires_at);
