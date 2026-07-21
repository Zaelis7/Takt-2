-- Requirements: PRD-IAM-001, PRD-IAM-004, PRD-IAM-005, PRD-DATA-001, PRD-DATA-002, PRD-DATA-004.
CREATE TABLE api_tokens (
    id UUID PRIMARY KEY CHECK (substring(id::text, 15, 1) = '7'),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NULL,
    name VARCHAR(120) NOT NULL CHECK (length(name) >= 1),
    kind VARCHAR(16) NOT NULL CHECK (kind IN ('personal', 'service')),
    token_prefix VARCHAR(32) NOT NULL UNIQUE
        CHECK (length(token_prefix) >= 8 AND token_prefix ~ '^takt_[A-Za-z0-9_-]+$'),
    token_hash TEXT NOT NULL CHECK (token_hash LIKE '$argon2id$%'),
    scopes JSONB NOT NULL CHECK (
        jsonb_typeof(scopes) = 'array' AND jsonb_array_length(scopes) BETWEEN 1 AND 100
    ),
    ip_networks JSONB NOT NULL DEFAULT '[]'::jsonb CHECK (
        jsonb_typeof(ip_networks) = 'array' AND jsonb_array_length(ip_networks) <= 32
    ),
    expires_at TIMESTAMPTZ(6) NULL,
    last_used_at TIMESTAMPTZ(6) NULL,
    revoked_at TIMESTAMPTZ(6) NULL,
    created_at TIMESTAMPTZ(6) NOT NULL,
    updated_at TIMESTAMPTZ(6) NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE CASCADE,
    CHECK (expires_at IS NULL OR expires_at > created_at),
    CHECK (last_used_at IS NULL OR last_used_at >= created_at),
    CHECK (revoked_at IS NULL OR revoked_at >= created_at),
    CHECK (updated_at >= created_at)
);
CREATE INDEX api_tokens_organization_page_idx
    ON api_tokens (organization_id, created_at DESC, id DESC);
CREATE INDEX api_tokens_project_idx ON api_tokens (project_id);
