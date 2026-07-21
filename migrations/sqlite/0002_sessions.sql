-- IAM-011: PRD-IAM-001/004/005, PRD-DATA-001/002/004, PRD-NFR-002/005.
CREATE TABLE sessions (
    id TEXT PRIMARY KEY
        CHECK (length(id) = 36 AND substr(id, 15, 1) = '7'),
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_digest TEXT NOT NULL UNIQUE
        CHECK (length(token_digest) = 71 AND substr(token_digest, 1, 7) = 'sha256:'
            AND substr(token_digest, 8) NOT GLOB '*[^0-9a-f]*'),
    csrf_digest TEXT NOT NULL UNIQUE
        CHECK (length(csrf_digest) = 71 AND substr(csrf_digest, 1, 7) = 'sha256:'
            AND substr(csrf_digest, 8) NOT GLOB '*[^0-9a-f]*'),
    issued_at INTEGER NOT NULL,
    last_activity_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    absolute_expires_at INTEGER NOT NULL,
    revoked_at INTEGER NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
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
