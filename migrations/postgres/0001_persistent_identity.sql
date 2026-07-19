CREATE TABLE organizations (
    id UUID PRIMARY KEY CHECK (substring(id::text, 15, 1) = '7'),
    slug VARCHAR(63) NOT NULL UNIQUE,
    name VARCHAR(120) NOT NULL,
    settings JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ(6) NOT NULL,
    updated_at TIMESTAMPTZ(6) NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    CHECK (slug ~ '^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$'),
    CHECK (updated_at >= created_at)
);

CREATE TABLE projects (
    id UUID PRIMARY KEY CHECK (substring(id::text, 15, 1) = '7'),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    slug VARCHAR(63) NOT NULL,
    name VARCHAR(120) NOT NULL,
    default_timezone VARCHAR(100) NOT NULL DEFAULT 'UTC',
    created_at TIMESTAMPTZ(6) NOT NULL,
    updated_at TIMESTAMPTZ(6) NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    UNIQUE (organization_id, slug),
    UNIQUE (organization_id, id),
    CHECK (slug ~ '^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$'),
    CHECK (updated_at >= created_at)
);

CREATE TABLE users (
    id UUID PRIMARY KEY CHECK (substring(id::text, 15, 1) = '7'),
    normalized_username VARCHAR(64) NOT NULL UNIQUE,
    display_name VARCHAR(120) NOT NULL,
    created_at TIMESTAMPTZ(6) NOT NULL,
    updated_at TIMESTAMPTZ(6) NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    CHECK (normalized_username ~ '^[a-z0-9](?:[a-z0-9_.-]{0,62}[a-z0-9])?$'),
    CHECK (normalized_username = lower(normalized_username)),
    CHECK (updated_at >= created_at)
);

CREATE TABLE local_credentials (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    password_hash TEXT NOT NULL CHECK (password_hash LIKE '$argon2id$%'),
    created_at TIMESTAMPTZ(6) NOT NULL,
    updated_at TIMESTAMPTZ(6) NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    CHECK (updated_at >= created_at)
);

CREATE TABLE memberships (
    id UUID PRIMARY KEY CHECK (substring(id::text, 15, 1) = '7'),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NULL,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(16) NOT NULL CHECK (role IN ('owner', 'admin', 'editor', 'operator', 'viewer')),
    created_at TIMESTAMPTZ(6) NOT NULL,
    updated_at TIMESTAMPTZ(6) NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE CASCADE,
    CHECK (updated_at >= created_at)
);

CREATE UNIQUE INDEX memberships_organization_user_unique
    ON memberships (organization_id, user_id) WHERE project_id IS NULL;
CREATE UNIQUE INDEX memberships_project_user_unique
    ON memberships (organization_id, project_id, user_id) WHERE project_id IS NOT NULL;

CREATE TABLE audit_events (
    id UUID PRIMARY KEY CHECK (substring(id::text, 15, 1) = '7'),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    project_id UUID NULL,
    actor_type VARCHAR(16) NOT NULL CHECK (actor_type IN ('system', 'local_cli')),
    actor_id UUID NULL REFERENCES users(id) ON DELETE RESTRICT,
    action VARCHAR(120) NOT NULL,
    resource_type VARCHAR(64) NOT NULL,
    resource_id UUID NOT NULL CHECK (substring(resource_id::text, 15, 1) = '7'),
    request_id UUID NOT NULL CHECK (substring(request_id::text, 15, 1) = '7'),
    source_ip_hash TEXT NULL,
    before_hash TEXT NULL,
    after_hash TEXT NULL,
    metadata JSONB NOT NULL,
    occurred_at TIMESTAMPTZ(6) NOT NULL,
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE RESTRICT
);

CREATE INDEX projects_organization_id_idx ON projects (organization_id);
CREATE INDEX memberships_organization_id_idx ON memberships (organization_id);
CREATE INDEX memberships_project_id_idx ON memberships (project_id);
CREATE INDEX memberships_user_id_idx ON memberships (user_id);
CREATE INDEX audit_events_organization_occurred_idx
    ON audit_events (organization_id, occurred_at DESC);
CREATE INDEX audit_events_project_occurred_idx
    ON audit_events (project_id, occurred_at DESC);
CREATE INDEX audit_events_request_id_idx ON audit_events (request_id);

CREATE FUNCTION takt_reject_audit_event_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'audit_events are append-only' USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER audit_events_reject_update
BEFORE UPDATE ON audit_events
FOR EACH ROW EXECUTE FUNCTION takt_reject_audit_event_mutation();

CREATE TRIGGER audit_events_reject_delete
BEFORE DELETE ON audit_events
FOR EACH ROW EXECUTE FUNCTION takt_reject_audit_event_mutation();
