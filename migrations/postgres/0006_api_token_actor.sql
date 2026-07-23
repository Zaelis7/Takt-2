-- IAM-038; Requirements: PRD-API-003, PRD-IAM-001, PRD-IAM-004,
-- PRD-IAM-005, PRD-DATA-001, PRD-DATA-002, PRD-DATA-004,
-- PRD-NFR-002, PRD-NFR-005.
ALTER TABLE audit_events
    ADD COLUMN api_token_actor_id UUID NULL
        REFERENCES api_tokens(id) ON DELETE RESTRICT;

ALTER TABLE audit_events
    DROP CONSTRAINT audit_events_actor_type_check;
ALTER TABLE audit_events
    ADD CONSTRAINT audit_events_actor_type_check
        CHECK (actor_type IN ('system', 'local_cli', 'api_token'));
ALTER TABLE audit_events
    ADD CONSTRAINT audit_events_actor_identity_check CHECK (
        (actor_type = 'api_token' AND actor_id IS NULL AND api_token_actor_id IS NOT NULL)
        OR (actor_type IN ('system', 'local_cli') AND api_token_actor_id IS NULL)
    );
CREATE INDEX audit_events_api_token_actor_idx
    ON audit_events (api_token_actor_id)
    WHERE api_token_actor_id IS NOT NULL;

ALTER TABLE api_token_idempotency
    DROP CONSTRAINT api_token_idempotency_actor_type_check;
ALTER TABLE api_token_idempotency
    ADD CONSTRAINT api_token_idempotency_actor_type_check
        CHECK (actor_type IN ('system', 'local_cli', 'api_token'));
