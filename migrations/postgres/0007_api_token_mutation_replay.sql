-- IAM-043; Requirements: PRD-API-002, PRD-API-003, PRD-API-005,
-- PRD-IAM-005, PRD-DATA-001, PRD-DATA-002, PRD-DATA-004.
ALTER TABLE api_token_idempotency
    ADD COLUMN response_status SMALLINT NULL
        CHECK (response_status IN (200, 204)),
    ADD COLUMN response_etag VARCHAR(64) NULL
        CHECK (
            response_etag IS NULL
            OR response_etag ~ '^"[1-9][0-9]*"$'
        ),
    ADD COLUMN mutation_snapshot JSONB NULL
        CHECK (
            mutation_snapshot IS NULL
            OR (
                jsonb_typeof(mutation_snapshot) = 'object'
                AND octet_length(mutation_snapshot::text) BETWEEN 2 AND 16384
                AND NOT (
                    mutation_snapshot
                    ?| ARRAY['token', 'token_hash', 'request_hash', 'replay_ciphertext']
                )
            )
        );

ALTER TABLE api_token_idempotency
    ADD CONSTRAINT api_token_idempotency_safe_response_check CHECK (
        (
            response_status IS NULL
            AND response_etag IS NULL
            AND mutation_snapshot IS NULL
        )
        OR (
            method = 'PATCH'
            AND api_token_id IS NOT NULL
            AND response_status = 200
            AND response_etag IS NOT NULL
            AND mutation_snapshot IS NOT NULL
        )
        OR (
            method = 'DELETE'
            AND api_token_id IS NOT NULL
            AND response_status = 204
            AND response_etag IS NULL
            AND mutation_snapshot IS NOT NULL
        )
    );
