CREATE TABLE recovery_tokens (
    id TEXT PRIMARY KEY
        CHECK (length(id) = 36 AND substr(id, 15, 1) = '7'),
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_digest TEXT NOT NULL UNIQUE
        CHECK (length(token_digest) = 71 AND substr(token_digest, 1, 7) = 'sha256:'
            AND substr(token_digest, 8) NOT GLOB '*[^0-9a-f]*'),
    expires_at INTEGER NOT NULL,
    consumed_at INTEGER NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
    CHECK (expires_at > created_at),
    CHECK (updated_at >= created_at),
    CHECK (consumed_at IS NULL OR (
        consumed_at >= created_at
        AND consumed_at < expires_at
        AND updated_at >= consumed_at
    ))
) STRICT;
CREATE INDEX recovery_tokens_user_pending_idx ON recovery_tokens (user_id, expires_at) WHERE consumed_at IS NULL;
