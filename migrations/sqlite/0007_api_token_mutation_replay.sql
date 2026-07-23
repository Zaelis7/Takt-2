-- IAM-043; Requirements: PRD-API-002, PRD-API-003, PRD-API-005,
-- PRD-IAM-005, PRD-DATA-001, PRD-DATA-002, PRD-DATA-004.
ALTER TABLE api_token_idempotency
    ADD COLUMN response_status INTEGER NULL
        CHECK (response_status IN (200, 204));
ALTER TABLE api_token_idempotency
    ADD COLUMN response_etag TEXT NULL
        CHECK (
            response_etag IS NULL
            OR (
                length(response_etag) BETWEEN 3 AND 64
                AND substr(response_etag, 1, 1) = '"'
                AND substr(response_etag, -1, 1) = '"'
                AND substr(response_etag, 2, 1) GLOB '[1-9]'
                AND substr(response_etag, 2, length(response_etag) - 2)
                    NOT GLOB '*[^0-9]*'
            )
        );
ALTER TABLE api_token_idempotency
    ADD COLUMN mutation_snapshot TEXT NULL
        CHECK (
            mutation_snapshot IS NULL
            OR CASE
                WHEN json_valid(mutation_snapshot) THEN
                    json_type(mutation_snapshot) = 'object'
                    AND length(mutation_snapshot) BETWEEN 2 AND 16384
                    AND json_type(mutation_snapshot, '$.token') IS NULL
                    AND json_type(mutation_snapshot, '$.token_hash') IS NULL
                    AND json_type(mutation_snapshot, '$.request_hash') IS NULL
                    AND json_type(mutation_snapshot, '$.replay_ciphertext') IS NULL
                ELSE 0
            END
        );

CREATE TRIGGER api_token_idempotency_safe_response_insert
BEFORE INSERT ON api_token_idempotency
WHEN NOT (
    (
        NEW.response_status IS NULL
        AND NEW.response_etag IS NULL
        AND NEW.mutation_snapshot IS NULL
    )
    OR (
        NEW.method = 'PATCH'
        AND NEW.api_token_id IS NOT NULL
        AND NEW.response_status = 200
        AND NEW.response_etag IS NOT NULL
        AND NEW.mutation_snapshot IS NOT NULL
    )
    OR (
        NEW.method = 'DELETE'
        AND NEW.api_token_id IS NOT NULL
        AND NEW.response_status = 204
        AND NEW.response_etag IS NULL
        AND NEW.mutation_snapshot IS NOT NULL
    )
)
BEGIN
    SELECT RAISE(ABORT, 'invalid API-token idempotency safe response');
END;

CREATE TRIGGER api_token_idempotency_safe_response_update
BEFORE UPDATE ON api_token_idempotency
WHEN NOT (
    (
        NEW.response_status IS NULL
        AND NEW.response_etag IS NULL
        AND NEW.mutation_snapshot IS NULL
    )
    OR (
        NEW.method = 'PATCH'
        AND NEW.api_token_id IS NOT NULL
        AND NEW.response_status = 200
        AND NEW.response_etag IS NOT NULL
        AND NEW.mutation_snapshot IS NOT NULL
    )
    OR (
        NEW.method = 'DELETE'
        AND NEW.api_token_id IS NOT NULL
        AND NEW.response_status = 204
        AND NEW.response_etag IS NULL
        AND NEW.mutation_snapshot IS NOT NULL
    )
)
BEGIN
    SELECT RAISE(ABORT, 'invalid API-token idempotency safe response');
END;
