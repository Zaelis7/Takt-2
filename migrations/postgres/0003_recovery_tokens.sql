CREATE TABLE recovery_tokens (
    id UUID PRIMARY KEY CHECK (substring(id::text, 15, 1) = '7'),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_digest VARCHAR(71) NOT NULL UNIQUE
        CHECK (token_digest ~ '^sha256:[0-9a-f]{64}$'),
    expires_at TIMESTAMPTZ(6) NOT NULL,
    consumed_at TIMESTAMPTZ(6) NULL,
    created_at TIMESTAMPTZ(6) NOT NULL,
    updated_at TIMESTAMPTZ(6) NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    CHECK (expires_at > created_at),
    CHECK (updated_at >= created_at),
    CHECK (consumed_at IS NULL OR (
        consumed_at >= created_at
        AND consumed_at < expires_at
        AND updated_at >= consumed_at
    ))
);
CREATE INDEX recovery_tokens_user_pending_idx ON recovery_tokens (user_id, expires_at) WHERE consumed_at IS NULL;
