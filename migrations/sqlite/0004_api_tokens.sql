-- Requirements: PRD-IAM-001, PRD-IAM-004, PRD-IAM-005, PRD-DATA-001, PRD-DATA-002, PRD-DATA-004.
CREATE TABLE api_tokens (
    id TEXT PRIMARY KEY CHECK (length(id) = 36 AND substr(id, 15, 1) = '7'),
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id TEXT NULL,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    kind TEXT NOT NULL CHECK (kind IN ('personal', 'service')),
    token_prefix TEXT NOT NULL UNIQUE CHECK (
        length(token_prefix) BETWEEN 8 AND 32
        AND substr(token_prefix, 1, 5) = 'takt_'
        AND substr(token_prefix, 6) NOT GLOB '*[^A-Za-z0-9_-]*'
    ),
    token_hash TEXT NOT NULL CHECK (token_hash LIKE '$argon2id$%'),
    scopes TEXT NOT NULL CHECK (
        json_valid(scopes) AND json_type(scopes) = 'array'
        AND json_array_length(scopes) BETWEEN 1 AND 100
    ),
    ip_networks TEXT NOT NULL DEFAULT '[]' CHECK (
        json_valid(ip_networks) AND json_type(ip_networks) = 'array'
        AND json_array_length(ip_networks) <= 32
    ),
    expires_at INTEGER NULL,
    last_used_at INTEGER NULL,
    revoked_at INTEGER NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE CASCADE,
    CHECK (expires_at IS NULL OR expires_at > created_at),
    CHECK (last_used_at IS NULL OR last_used_at >= created_at),
    CHECK (revoked_at IS NULL OR revoked_at >= created_at),
    CHECK (updated_at >= created_at)
) STRICT;
CREATE INDEX api_tokens_organization_page_idx
    ON api_tokens (organization_id, created_at DESC, id DESC);
CREATE INDEX api_tokens_project_idx ON api_tokens (project_id);
