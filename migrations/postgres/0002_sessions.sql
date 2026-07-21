-- IAM-011: PRD-IAM-001/004/005, PRD-DATA-001/002/004, PRD-NFR-002/005.
CREATE TABLE sessions (
    id UUID PRIMARY KEY CHECK (substring(id::text, 15, 1) = '7'),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_digest VARCHAR(71) NOT NULL UNIQUE
        CHECK (token_digest ~ '^sha256:[0-9a-f]{64}$'),
    csrf_digest VARCHAR(71) NOT NULL UNIQUE
        CHECK (csrf_digest ~ '^sha256:[0-9a-f]{64}$'),
    issued_at TIMESTAMPTZ(6) NOT NULL,
    last_activity_at TIMESTAMPTZ(6) NOT NULL,
    expires_at TIMESTAMPTZ(6) NOT NULL,
    absolute_expires_at TIMESTAMPTZ(6) NOT NULL,
    revoked_at TIMESTAMPTZ(6) NULL,
    created_at TIMESTAMPTZ(6) NOT NULL,
    updated_at TIMESTAMPTZ(6) NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    CHECK (token_digest <> csrf_digest),
    CHECK (last_activity_at >= issued_at),
    CHECK (expires_at > last_activity_at),
    CHECK (absolute_expires_at >= expires_at),
    CHECK (created_at = issued_at),
    CHECK (updated_at >= created_at),
    CHECK (revoked_at IS NULL OR (revoked_at >= issued_at AND revoked_at <= updated_at))
);
CREATE INDEX sessions_user_active_idx
    ON sessions (user_id, absolute_expires_at)
    WHERE revoked_at IS NULL;
