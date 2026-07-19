CREATE TABLE organizations (
    id TEXT PRIMARY KEY CHECK (length(id) = 36 AND substr(id, 15, 1) = '7'),
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    settings TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(settings)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
    CHECK (
        length(slug) BETWEEN 1 AND 63
        AND slug NOT GLOB '*[^a-z0-9-]*'
        AND substr(slug, 1, 1) GLOB '[a-z0-9]'
        AND substr(slug, -1, 1) GLOB '[a-z0-9]'
    ),
    CHECK (updated_at >= created_at)
) STRICT;

CREATE TABLE projects (
    id TEXT PRIMARY KEY CHECK (length(id) = 36 AND substr(id, 15, 1) = '7'),
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    slug TEXT NOT NULL,
    name TEXT NOT NULL,
    default_timezone TEXT NOT NULL DEFAULT 'UTC',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
    UNIQUE (organization_id, slug),
    UNIQUE (organization_id, id),
    CHECK (
        length(slug) BETWEEN 1 AND 63
        AND slug NOT GLOB '*[^a-z0-9-]*'
        AND substr(slug, 1, 1) GLOB '[a-z0-9]'
        AND substr(slug, -1, 1) GLOB '[a-z0-9]'
    ),
    CHECK (updated_at >= created_at)
) STRICT;

CREATE TABLE users (
    id TEXT PRIMARY KEY CHECK (length(id) = 36 AND substr(id, 15, 1) = '7'),
    normalized_username TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
    CHECK (
        length(normalized_username) BETWEEN 1 AND 64
        AND normalized_username NOT GLOB '*[^a-z0-9_.-]*'
        AND substr(normalized_username, 1, 1) GLOB '[a-z0-9]'
        AND substr(normalized_username, -1, 1) GLOB '[a-z0-9]'
    ),
    CHECK (normalized_username = lower(normalized_username)),
    CHECK (updated_at >= created_at)
) STRICT;

CREATE TABLE local_credentials (
    user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    password_hash TEXT NOT NULL CHECK (password_hash LIKE '$argon2id$%'),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
    CHECK (updated_at >= created_at)
) STRICT;

CREATE TABLE memberships (
    id TEXT PRIMARY KEY CHECK (length(id) = 36 AND substr(id, 15, 1) = '7'),
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id TEXT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'editor', 'operator', 'viewer')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE CASCADE,
    CHECK (updated_at >= created_at)
) STRICT;

CREATE UNIQUE INDEX memberships_organization_user_unique
    ON memberships (organization_id, user_id) WHERE project_id IS NULL;
CREATE UNIQUE INDEX memberships_project_user_unique
    ON memberships (organization_id, project_id, user_id) WHERE project_id IS NOT NULL;

CREATE TABLE audit_events (
    id TEXT PRIMARY KEY CHECK (length(id) = 36 AND substr(id, 15, 1) = '7'),
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    project_id TEXT NULL,
    actor_type TEXT NOT NULL CHECK (actor_type IN ('system', 'local_cli')),
    actor_id TEXT NULL REFERENCES users(id) ON DELETE RESTRICT,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL CHECK (length(resource_id) = 36 AND substr(resource_id, 15, 1) = '7'),
    request_id TEXT NOT NULL CHECK (length(request_id) = 36 AND substr(request_id, 15, 1) = '7'),
    source_ip_hash TEXT NULL,
    before_hash TEXT NULL,
    after_hash TEXT NULL,
    metadata TEXT NOT NULL CHECK (json_valid(metadata)),
    occurred_at INTEGER NOT NULL,
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE RESTRICT
) STRICT;

CREATE INDEX projects_organization_id_idx ON projects (organization_id);
CREATE INDEX memberships_organization_id_idx ON memberships (organization_id);
CREATE INDEX memberships_project_id_idx ON memberships (project_id);
CREATE INDEX memberships_user_id_idx ON memberships (user_id);
CREATE INDEX audit_events_organization_occurred_idx
    ON audit_events (organization_id, occurred_at DESC);
CREATE INDEX audit_events_project_occurred_idx
    ON audit_events (project_id, occurred_at DESC);
CREATE INDEX audit_events_request_id_idx ON audit_events (request_id);

CREATE TRIGGER audit_events_reject_update
BEFORE UPDATE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit_events are append-only');
END;

CREATE TRIGGER audit_events_reject_delete
BEFORE DELETE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit_events are append-only');
END;
