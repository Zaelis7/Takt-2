export interface paths {
    "/health/live": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Check whether the server process is alive */
        get: operations["getLiveness"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/health/ready": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Check whether the instance is ready to serve traffic */
        get: operations["getReadiness"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/system/info": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Get non-secret instance capabilities */
        get: operations["getSystemInfo"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/auth/login": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Start a browser session with local credentials
         * @description Invalid credentials use an identical response for an unknown username and an invalid password.
         */
        post: operations["login"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/auth/logout": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** Revoke the current browser session */
        post: operations["logout"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/auth/session": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Get the current browser session and CSRF token */
        get: operations["getSession"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/auth/recovery/request": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Request a one-time password recovery token
         * @description The response is identical whether or not the account exists.
         */
        post: operations["requestPasswordRecovery"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/auth/recovery/complete": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** Replace a local password with a one-time recovery token */
        post: operations["completePasswordRecovery"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/api-tokens": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * List redacted API-token metadata
         * @description Results use stable ordering by created_at descending and then id descending.
         */
        get: operations["listApiTokens"];
        put?: never;
        /**
         * Create an API token and reveal its value once
         * @description An identical idempotency replay returns the same 201 response with the same token for 24 hours; the opaque value is otherwise returned only by this operation and cannot be retrieved later.
         */
        post: operations["createApiToken"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/api-tokens/{api_token_id}": {
        parameters: {
            query?: never;
            header?: never;
            path: {
                api_token_id: components["parameters"]["ApiTokenId"];
            };
            cookie?: never;
        };
        /** Get redacted API-token metadata */
        get: operations["getApiToken"];
        put?: never;
        post?: never;
        /**
         * Revoke an API token
         * @description An identical replay returns the original empty 204 response for 24 hours even if the token resource changes later.
         */
        delete: operations["revokeApiToken"];
        options?: never;
        head?: never;
        /**
         * Update non-privilege-bearing API-token metadata
         * @description Scopes, kind, organization and project are immutable; replace the token to change its authorization. An identical replay returns the original 200 response, ETag and safe redacted body for 24 hours even if the token was mutated again later.
         */
        patch: operations["updateApiToken"];
        trace?: never;
    };
    "/api/v1/projects/{project_id}/monitors": {
        parameters: {
            query?: never;
            header?: never;
            path: {
                project_id: components["parameters"]["ProjectId"];
            };
            cookie?: never;
        };
        /** List monitors in a project */
        get: operations["listMonitors"];
        put?: never;
        /** Create a monitor */
        post: operations["createMonitor"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{project_id}/monitors/{monitor_id}": {
        parameters: {
            query?: never;
            header?: never;
            path: {
                project_id: components["parameters"]["ProjectId"];
                monitor_id: components["parameters"]["MonitorId"];
            };
            cookie?: never;
        };
        /** Get a monitor */
        get: operations["getMonitor"];
        put?: never;
        post?: never;
        /** Delete a monitor */
        delete: operations["deleteMonitor"];
        options?: never;
        head?: never;
        /** Update selected monitor fields */
        patch: operations["updateMonitor"];
        trace?: never;
    };
    "/api/v1/projects/{project_id}/monitors/{monitor_id}/checks": {
        parameters: {
            query?: never;
            header?: never;
            path: {
                project_id: components["parameters"]["ProjectId"];
                monitor_id: components["parameters"]["MonitorId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** Queue an immediate monitor check */
        post: operations["runMonitorCheck"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{project_id}/monitors/{monitor_id}/observations": {
        parameters: {
            query?: never;
            header?: never;
            path: {
                project_id: components["parameters"]["ProjectId"];
                monitor_id: components["parameters"]["MonitorId"];
            };
            cookie?: never;
        };
        /** List recent monitor observations */
        get: operations["listMonitorObservations"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{project_id}/monitors/{monitor_id}/uptime": {
        parameters: {
            query: {
                from: string;
                to: string;
                bucket?: "raw" | "minute" | "hour" | "day";
            };
            header?: never;
            path: {
                project_id: components["parameters"]["ProjectId"];
                monitor_id: components["parameters"]["MonitorId"];
            };
            cookie?: never;
        };
        /** Get time-weighted monitor uptime */
        get: operations["getMonitorUptime"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/probes": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** List remote probes */
        get: operations["listProbes"];
        put?: never;
        /** Create a one-time probe enrollment */
        post: operations["createProbeEnrollment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/config/validate": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** Validate a declarative configuration */
        post: operations["validateConfig"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/config/plan": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** Plan a declarative configuration change */
        post: operations["planConfig"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/config/apply": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** Apply a declarative configuration */
        post: operations["applyConfig"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/public/v1/status-pages/{slug}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Get a redacted public status page projection */
        get: operations["getPublicStatusPage"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        /** Format: uuid */
        Uuid: string;
        Slug: string;
        Health: {
            /** @enum {string} */
            status: "ok";
        };
        SystemInfo: {
            version: string;
            /** @enum {string} */
            database: "postgresql" | "sqlite";
            capabilities: string[];
        };
        LoginRequest: {
            username: string;
            /** @description Password input; the runtime enforces a maximum of 1024 UTF-8 bytes. */
            password: string;
        };
        PasswordRecoveryRequest: {
            username: string;
        };
        PasswordRecoveryComplete: {
            /** @description Opaque, one-time and short-lived recovery token. */
            token: string;
            /** @description Replacement password; the runtime enforces a maximum of 1024 UTF-8 bytes. */
            new_password: string;
        };
        SessionUser: {
            id: components["schemas"]["Uuid"];
            username: string;
            display_name: string;
        };
        Session: {
            user: components["schemas"]["SessionUser"];
            permissions: string[];
            /** @description Session-bound token for the X-CSRF-Token request header. */
            csrf_token: string;
            /**
             * Format: date-time
             * @description Inactivity expiry, 12 hours by default and configurable by the operator.
             */
            expires_at: string;
            /**
             * Format: date-time
             * @description Absolute expiry, 7 days by default and configurable by the operator.
             */
            absolute_expires_at: string;
        };
        /** @enum {string} */
        ApiTokenKind: "personal" | "service";
        /** @enum {string} */
        ApiTokenStatus: "active" | "revoked" | "expired";
        ApiTokenScope: string;
        /** @description Canonical IPv4 or IPv6 CIDR validated by the runtime. */
        IpNetwork: string;
        ApiTokenCreate: {
            name: string;
            kind: components["schemas"]["ApiTokenKind"];
            scopes: components["schemas"]["ApiTokenScope"][];
            project_id?: components["schemas"]["Uuid"];
            /** Format: date-time */
            expires_at?: string;
            ip_networks?: components["schemas"]["IpNetwork"][];
        };
        ApiTokenPatch: {
            name?: string;
            expires_at?: string | null;
            ip_networks?: components["schemas"]["IpNetwork"][];
        };
        ApiToken: {
            id: components["schemas"]["Uuid"];
            organization_id: components["schemas"]["Uuid"];
            project_id: components["schemas"]["Uuid"] | null;
            name: string;
            kind: components["schemas"]["ApiTokenKind"];
            /** @description Non-secret lookup prefix; never sufficient to authenticate. */
            token_prefix: string;
            scopes: components["schemas"]["ApiTokenScope"][];
            ip_networks: components["schemas"]["IpNetwork"][];
            status: components["schemas"]["ApiTokenStatus"];
            expires_at: string | null;
            last_used_at: string | null;
            revoked_at: string | null;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
            version: number;
        };
        ApiTokenCreated: {
            /** @description Opaque bearer token returned only in the successful creation response. */
            readonly token: string;
            api_token: components["schemas"]["ApiToken"];
        };
        ApiTokenPage: {
            items: components["schemas"]["ApiToken"][];
            next_cursor: string | null;
        };
        /** @enum {string} */
        MonitorKind: "http" | "tcp" | "dns" | "icmp" | "tls" | "push" | "browser";
        /** @enum {string} */
        MonitorState: "PENDING" | "UP" | "DEGRADED" | "DOWN" | "PAUSED" | "MAINTENANCE" | "UNKNOWN";
        ProbePolicy: {
            /** @enum {string} */
            mode: "any" | "all" | "quorum";
            min_success?: number;
        } & unknown;
        MonitorCommon: {
            slug: components["schemas"]["Slug"];
            name: string;
            description?: string | null;
            kind: components["schemas"]["MonitorKind"];
            enabled: boolean;
            schedule_interval_ms: number;
            timeout_ms: number;
            failure_threshold: number;
            recovery_threshold: number;
            probe_policy?: components["schemas"]["ProbePolicy"];
            probe_selector?: {
                [key: string]: string;
            };
            tags?: string[];
            spec: components["schemas"]["CheckSpec"];
        };
        MonitorCreate: components["schemas"]["MonitorCommon"];
        MonitorPatch: {
            name?: string;
            description?: string | null;
            enabled?: boolean;
            schedule_interval_ms?: number;
            timeout_ms?: number;
            failure_threshold?: number;
            recovery_threshold?: number;
            probe_policy?: components["schemas"]["ProbePolicy"];
            probe_selector?: {
                [key: string]: string;
            };
            tags?: string[];
            spec?: components["schemas"]["CheckSpec"];
        };
        Monitor: components["schemas"]["MonitorCommon"] & {
            id: components["schemas"]["Uuid"];
            project_id: components["schemas"]["Uuid"];
            state: components["schemas"]["MonitorState"];
            /** Format: date-time */
            state_since: string;
            version: number;
            /** @enum {string} */
            managed_by?: "manual" | "declarative" | "terraform" | "discovery";
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            updated_at: string;
        };
        MonitorPage: {
            items: components["schemas"]["Monitor"][];
            next_cursor: string | null;
        };
        CheckSpec: components["schemas"]["HttpCheckSpec"] | components["schemas"]["TcpCheckSpec"] | components["schemas"]["DnsCheckSpec"] | components["schemas"]["IcmpCheckSpec"] | components["schemas"]["TlsCheckSpec"] | components["schemas"]["PushCheckSpec"] | components["schemas"]["BrowserCheckSpec"];
        /** @description Persistent secret reference resolved only at the execution boundary; secret values are never returned. */
        SecretRef: {
            secret_ref: components["schemas"]["Slug"];
            /** @default value */
            key: string;
        };
        /**
         * Format: uri
         * @description Authority-only DNS resolver URI without credentials, path, query, or fragment.
         */
        ResolverUri: string;
        /**
         * @description Address family used to connect to the target, resolver, or proxy; it does not change a DNS record type.
         * @default auto
         * @enum {string}
         */
        AddressFamily: "auto" | "ipv4" | "ipv6";
        ProxyBasicAuth: {
            username: components["schemas"]["SecretRef"];
            password: components["schemas"]["SecretRef"];
        };
        ProxySpec: {
            /**
             * Format: uri
             * @description Authority-only proxy URL; credentials are forbidden in the URL.
             */
            url: string;
            auth?: components["schemas"]["ProxyBasicAuth"];
        };
        HttpValue: string | components["schemas"]["SecretRef"];
        JsonPointerAssertion: {
            pointer: string;
            value: string;
        };
        HttpBasicAuth: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            type: "HttpBasicAuth";
            username: components["schemas"]["SecretRef"];
            password: components["schemas"]["SecretRef"];
        };
        HttpBearerAuth: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            type: "HttpBearerAuth";
            token: components["schemas"]["SecretRef"];
        };
        HttpMtlsAuth: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            type: "HttpMtlsAuth";
            client_certificate: components["schemas"]["SecretRef"];
            client_key: components["schemas"]["SecretRef"];
        };
        HttpAuth: components["schemas"]["HttpBasicAuth"] | components["schemas"]["HttpBearerAuth"] | components["schemas"]["HttpMtlsAuth"];
        HttpCheckSpec: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            type: "http";
            /** Format: uri */
            url: string;
            /**
             * @default GET
             * @enum {string}
             */
            method: "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS";
            headers?: {
                [key: string]: string | components["schemas"]["SecretRef"];
            };
            body?: components["schemas"]["HttpValue"];
            /** @default 200 */
            expected_status_min: number;
            /** @default 399 */
            expected_status_max: number;
            /** @default 5 */
            follow_redirects: number;
            /** @default true */
            verify_tls: boolean;
            /**
             * @default auto
             * @enum {string}
             */
            http_version: "auto" | "http1_1" | "http2";
            body_contains?: string;
            body_matches?: string;
            json_pointer_equals?: components["schemas"]["JsonPointerAssertion"];
            json_pointer_contains?: components["schemas"]["JsonPointerAssertion"];
            max_response_time_ms?: number;
            /** @default 1048576 */
            response_body_limit_bytes: number;
            auth?: components["schemas"]["HttpAuth"];
            proxy?: components["schemas"]["ProxySpec"];
            resolver?: components["schemas"]["ResolverUri"];
            address_family?: components["schemas"]["AddressFamily"];
        };
        TcpCheckSpec: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            type: "tcp";
            host: string;
            port: number;
            /** @description UTF-8 text encoded to send_bytes for the Probe contract; the runtime enforces the 4096-byte limit. */
            send_text?: string;
            /** @description UTF-8 text encoded to bytes for the Probe contract; the runtime enforces the 4096-byte limit. */
            expect_prefix?: string;
            proxy?: components["schemas"]["ProxySpec"];
            resolver?: components["schemas"]["ResolverUri"];
            address_family?: components["schemas"]["AddressFamily"];
        };
        DnsCheckSpec: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            type: "dns";
            name: string;
            /** @enum {string} */
            record_type: "A" | "AAAA" | "CNAME" | "MX" | "TXT" | "NS" | "SOA" | "CAA";
            expected_values?: string[];
            resolver?: components["schemas"]["ResolverUri"];
            address_family?: components["schemas"]["AddressFamily"];
            /**
             * @default NOERROR
             * @enum {string}
             */
            expected_rcode: "NOERROR" | "NXDOMAIN" | "SERVFAIL" | "REFUSED";
            /** @default 1 */
            minimum_answers: number;
            /**
             * @default contains
             * @enum {string}
             */
            value_match: "exact" | "contains";
        };
        IcmpCheckSpec: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            type: "icmp";
            host: string;
            /** @default 3 */
            packets: number;
            /** @default 1 */
            required_successes: number;
            max_latency_ms?: number;
            resolver?: components["schemas"]["ResolverUri"];
            address_family?: components["schemas"]["AddressFamily"];
        };
        TlsCheckSpec: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            type: "tls";
            host: string;
            /** @default 443 */
            port: number;
            server_name?: string;
            /** @default 30 */
            warning_days: number;
            /** @default 7 */
            critical_days: number;
            proxy?: components["schemas"]["ProxySpec"];
            resolver?: components["schemas"]["ResolverUri"];
            address_family?: components["schemas"]["AddressFamily"];
        };
        PushCheckSpec: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            type: "push";
            /** @default 60000 */
            grace_ms: number;
            /** @default false */
            allow_get: boolean;
        };
        BrowserCheckSpec: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            type: "browser";
            /** Format: uri */
            start_url: string;
            steps: ({
                /** @enum {string} */
                action: "navigate" | "click" | "fill" | "wait" | "assert_text" | "assert_url" | "assert_status";
                selector?: string;
                value?: string | components["schemas"]["SecretRef"];
            } & unknown)[];
            /** @default 10485760 */
            max_network_response_bytes: number;
            /** @default 1048576 */
            screenshot_on_failure_max_bytes: number;
            proxy?: components["schemas"]["ProxySpec"];
            resolver?: components["schemas"]["ResolverUri"];
            address_family?: components["schemas"]["AddressFamily"];
        };
        /** @enum {string} */
        ObservationOutcome: "SUCCESS" | "TARGET_FAILURE" | "PROBE_FAILURE" | "CANCELLED";
        Observation: {
            id: components["schemas"]["Uuid"];
            monitor_id: components["schemas"]["Uuid"];
            /** Format: uuid */
            probe_id?: string | null;
            outcome: components["schemas"]["ObservationOutcome"];
            error_code?: string | null;
            summary?: string | null;
            /** Format: date-time */
            started_at: string;
            /** Format: date-time */
            finished_at: string;
            duration_ms: number;
            late: boolean;
        };
        ObservationPage: {
            items: components["schemas"]["Observation"][];
            next_cursor: string | null;
        };
        UptimeSummary: {
            /** Format: date-time */
            from: string;
            /** Format: date-time */
            to: string;
            total_ms: number;
            included_ms: number;
            uptime_ratio: number;
            degraded_ratio: number;
            excluded_ratio: number;
            buckets: {
                /** Format: date-time */
                from: string;
                /** Format: date-time */
                to: string;
                state: components["schemas"]["MonitorState"];
            }[];
        };
        Probe: {
            id: components["schemas"]["Uuid"];
            slug: components["schemas"]["Slug"];
            display_name: string;
            labels: {
                [key: string]: string;
            };
            /** @enum {string} */
            status: "pending" | "connected" | "disconnected" | "offline" | "revoked" | "incompatible";
            /** Format: date-time */
            last_seen_at?: string | null;
            version: string;
        };
        ProbePage: {
            items: components["schemas"]["Probe"][];
            next_cursor: string | null;
        };
        ProbeEnrollmentCreate: {
            slug: components["schemas"]["Slug"];
            display_name: string;
            labels?: {
                [key: string]: string;
            };
        };
        ProbeEnrollment: {
            probe_id: components["schemas"]["Uuid"];
            enrollment_code: string;
            /** Format: date-time */
            expires_at: string;
            /** Format: uri */
            server_url: string;
        };
        Operation: {
            id: components["schemas"]["Uuid"];
            kind: string;
            /** @enum {string} */
            status: "queued" | "running" | "succeeded" | "failed" | "cancelled";
            progress?: number | null;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            completed_at?: string | null;
        };
        ValidationResult: {
            valid: boolean;
            warnings: components["schemas"]["FieldIssue"][];
        };
        ConfigPlan: {
            plan_id: components["schemas"]["Uuid"];
            creates: components["schemas"]["PlanChange"][];
            updates: components["schemas"]["PlanChange"][];
            deletes: components["schemas"]["PlanChange"][];
            unchanged: number;
            warnings: components["schemas"]["FieldIssue"][];
        };
        PlanChange: {
            resource_type: string;
            slug: components["schemas"]["Slug"];
            changes?: {
                path: string;
                before?: unknown;
                after?: unknown;
                secret: boolean;
            }[];
        };
        PublicStatusPage: {
            slug: components["schemas"]["Slug"];
            title: string;
            /** @enum {string} */
            overall_state: "operational" | "degraded" | "major_outage" | "maintenance" | "unknown";
            components: {
                name: string;
                state: components["schemas"]["MonitorState"];
                uptime_ratio?: number | null;
            }[];
            incidents: {
                id: components["schemas"]["Uuid"];
                title: string;
                /** @enum {string} */
                status: "INVESTIGATING" | "IDENTIFIED" | "MONITORING" | "RESOLVED";
                /** @enum {string} */
                impact: "none" | "minor" | "major" | "critical";
                /** Format: date-time */
                updated_at: string;
            }[];
            /** Format: date-time */
            updated_at: string;
        };
        FieldIssue: {
            path: string;
            code: string;
            message: string;
        };
        Problem: {
            /** Format: uri */
            type: string;
            title: string;
            status: number;
            code: string;
            detail?: string;
            instance?: string;
            request_id: string;
            errors?: components["schemas"]["FieldIssue"][];
        } & {
            [key: string]: unknown;
        };
        InvalidRequestProblem: components["schemas"]["Problem"] & {
            /** @constant */
            status?: 400;
            /** @constant */
            code?: "invalid_request";
        };
        InvalidCursorProblem: components["schemas"]["Problem"] & {
            /** @constant */
            status?: 400;
            /** @constant */
            code?: "invalid_cursor";
        };
        IdempotencyKeyReusedProblem: components["schemas"]["Problem"] & {
            /** @constant */
            status?: 409;
            /** @constant */
            code?: "idempotency_key_reused";
        };
        AuthenticationProblem: components["schemas"]["Problem"] & {
            /** @constant */
            status?: 401;
            /** @constant */
            code?: "authentication_failed";
        };
        CsrfProblem: components["schemas"]["Problem"] & {
            /** @constant */
            status?: 403;
            /** @constant */
            code?: "csrf_failed";
        };
        RecoveryProblem: components["schemas"]["Problem"] & {
            /** @constant */
            status?: 400;
            /** @constant */
            code?: "recovery_failed";
        };
        ValidationProblem: components["schemas"]["Problem"] & {
            /** @constant */
            status?: 422;
            /** @constant */
            code?: "validation_failed";
        };
        RateLimitProblem: components["schemas"]["Problem"] & {
            /** @constant */
            code?: "rate_limit_exceeded";
        };
    };
    responses: {
        /** @description Problem Details response */
        Problem: {
            headers: {
                "X-Request-Id": components["headers"]["RequestId"];
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["Problem"];
            };
        };
        /** @description The same actor reused an Idempotency-Key for the same method and path with a different request hash */
        IdempotencyKeyReusedProblem: {
            headers: {
                "X-Request-Id": components["headers"]["RequestId"];
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["IdempotencyKeyReusedProblem"];
            };
        };
        /** @description The request body is malformed or contains unknown fields */
        InvalidRequestProblem: {
            headers: {
                "X-Request-Id": components["headers"]["RequestId"];
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["InvalidRequestProblem"];
            };
        };
        /** @description The list cursor is invalid, expired or does not match the active filters and sort */
        InvalidCursorProblem: {
            headers: {
                "X-Request-Id": components["headers"]["RequestId"];
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["InvalidCursorProblem"];
            };
        };
        /** @description Authentication failed without disclosing account or session details */
        AuthenticationProblem: {
            headers: {
                "X-Request-Id": components["headers"]["RequestId"];
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["AuthenticationProblem"];
            };
        };
        /** @description The session-bound CSRF proof is missing or invalid */
        CsrfProblem: {
            headers: {
                "X-Request-Id": components["headers"]["RequestId"];
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["CsrfProblem"];
            };
        };
        /** @description The recovery request is malformed or its token cannot be accepted */
        RecoveryCompletionProblem: {
            headers: {
                "X-Request-Id": components["headers"]["RequestId"];
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["InvalidRequestProblem"] | components["schemas"]["RecoveryProblem"];
            };
        };
        /** @description One or more bounded input fields are invalid */
        ValidationProblem: {
            headers: {
                "X-Request-Id": components["headers"]["RequestId"];
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["ValidationProblem"];
            };
        };
        /** @description Rate limit exceeded */
        RateLimitProblem: {
            headers: {
                "X-Request-Id": components["headers"]["RequestId"];
                "Retry-After": components["headers"]["RetryAfter"];
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["RateLimitProblem"];
            };
        };
    };
    parameters: {
        ApiTokenId: components["schemas"]["Uuid"];
        ProjectId: components["schemas"]["Uuid"];
        MonitorId: components["schemas"]["Uuid"];
        Limit: number;
        Cursor: string;
        IdempotencyKey: string;
        IfMatch: string;
        /** @description Session-bound token required for browser state changes. */
        CsrfToken: string;
        /** @description Required when the operation is authorized with the browser session cookie; ignored for bearer authentication. */
        CsrfTokenIfSession: string;
    };
    requestBodies: never;
    headers: {
        /** @description Quoted resource version */
        ETag: string;
        /** @description Rotated opaque session cookie using HttpOnly, SameSite=Lax, Path=/ and Secure outside explicit localhost mode. */
        SessionCookie: string;
        /** @description Expired HttpOnly, SameSite=Lax, Path=/ session cookie, retaining Secure outside explicit localhost mode, used to clear browser state. */
        ExpiredSessionCookie: string;
        /** @description UUIDv7 request correlation identifier. */
        RequestId: components["schemas"]["Uuid"];
        /** @description Seconds until the client should retry. */
        RetryAfter: number;
    };
    pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
    getLiveness: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Process is alive */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Health"];
                };
            };
            429: components["responses"]["Problem"];
        };
    };
    getReadiness: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Instance is ready */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Health"];
                };
            };
            429: components["responses"]["Problem"];
            503: components["responses"]["Problem"];
        };
    };
    getSystemInfo: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Non-secret system capabilities */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SystemInfo"];
                };
            };
            401: components["responses"]["Problem"];
        };
    };
    login: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["LoginRequest"];
            };
        };
        responses: {
            /** @description Authenticated session with a rotated session cookie */
            200: {
                headers: {
                    "Set-Cookie": components["headers"]["SessionCookie"];
                    "X-Request-Id": components["headers"]["RequestId"];
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Session"];
                };
            };
            400: components["responses"]["InvalidRequestProblem"];
            401: components["responses"]["AuthenticationProblem"];
            422: components["responses"]["ValidationProblem"];
            429: components["responses"]["RateLimitProblem"];
        };
    };
    logout: {
        parameters: {
            query?: never;
            header: {
                /** @description Session-bound token required for browser state changes. */
                "X-CSRF-Token": components["parameters"]["CsrfToken"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Session revoked */
            204: {
                headers: {
                    "Set-Cookie": components["headers"]["ExpiredSessionCookie"];
                    "X-Request-Id": components["headers"]["RequestId"];
                    [name: string]: unknown;
                };
                content?: never;
            };
            401: components["responses"]["AuthenticationProblem"];
            403: components["responses"]["CsrfProblem"];
            429: components["responses"]["RateLimitProblem"];
        };
    };
    getSession: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Current browser session */
            200: {
                headers: {
                    "X-Request-Id": components["headers"]["RequestId"];
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Session"];
                };
            };
            401: components["responses"]["AuthenticationProblem"];
            429: components["responses"]["RateLimitProblem"];
        };
    };
    requestPasswordRecovery: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PasswordRecoveryRequest"];
            };
        };
        responses: {
            /** @description Recovery request accepted without disclosing account existence */
            202: {
                headers: {
                    "X-Request-Id": components["headers"]["RequestId"];
                    [name: string]: unknown;
                };
                content?: never;
            };
            400: components["responses"]["InvalidRequestProblem"];
            422: components["responses"]["ValidationProblem"];
            429: components["responses"]["RateLimitProblem"];
        };
    };
    completePasswordRecovery: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PasswordRecoveryComplete"];
            };
        };
        responses: {
            /** @description Password replaced and existing sessions revoked */
            204: {
                headers: {
                    "X-Request-Id": components["headers"]["RequestId"];
                    [name: string]: unknown;
                };
                content?: never;
            };
            400: components["responses"]["RecoveryCompletionProblem"];
            422: components["responses"]["ValidationProblem"];
            429: components["responses"]["RateLimitProblem"];
        };
    };
    listApiTokens: {
        parameters: {
            query?: {
                limit?: components["parameters"]["Limit"];
                cursor?: components["parameters"]["Cursor"];
                project_id?: components["schemas"]["Uuid"];
                kind?: components["schemas"]["ApiTokenKind"];
                status?: components["schemas"]["ApiTokenStatus"];
                scope?: components["schemas"]["ApiTokenScope"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Stable cursor page containing redacted token metadata */
            200: {
                headers: {
                    "X-Request-Id": components["headers"]["RequestId"];
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiTokenPage"];
                };
            };
            400: components["responses"]["InvalidCursorProblem"];
            401: components["responses"]["AuthenticationProblem"];
            403: components["responses"]["Problem"];
        };
    };
    createApiToken: {
        parameters: {
            query?: never;
            header?: {
                "Idempotency-Key"?: components["parameters"]["IdempotencyKey"];
                /** @description Required when the operation is authorized with the browser session cookie; ignored for bearer authentication. */
                "X-CSRF-Token"?: components["parameters"]["CsrfTokenIfSession"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ApiTokenCreate"];
            };
        };
        responses: {
            /** @description Token created; this is the only response containing its opaque value */
            201: {
                headers: {
                    ETag: components["headers"]["ETag"];
                    Location?: string;
                    "X-Request-Id": components["headers"]["RequestId"];
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiTokenCreated"];
                };
            };
            400: components["responses"]["InvalidRequestProblem"];
            401: components["responses"]["AuthenticationProblem"];
            403: components["responses"]["Problem"];
            409: components["responses"]["IdempotencyKeyReusedProblem"];
            422: components["responses"]["ValidationProblem"];
        };
    };
    getApiToken: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                api_token_id: components["parameters"]["ApiTokenId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Redacted token metadata */
            200: {
                headers: {
                    ETag: components["headers"]["ETag"];
                    "X-Request-Id": components["headers"]["RequestId"];
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiToken"];
                };
            };
            401: components["responses"]["AuthenticationProblem"];
            403: components["responses"]["Problem"];
            404: components["responses"]["Problem"];
        };
    };
    revokeApiToken: {
        parameters: {
            query?: never;
            header?: {
                "If-Match"?: components["parameters"]["IfMatch"];
                "Idempotency-Key"?: components["parameters"]["IdempotencyKey"];
                /** @description Required when the operation is authorized with the browser session cookie; ignored for bearer authentication. */
                "X-CSRF-Token"?: components["parameters"]["CsrfTokenIfSession"];
            };
            path: {
                api_token_id: components["parameters"]["ApiTokenId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Token revoked */
            204: {
                headers: {
                    "X-Request-Id": components["headers"]["RequestId"];
                    [name: string]: unknown;
                };
                content?: never;
            };
            401: components["responses"]["AuthenticationProblem"];
            403: components["responses"]["Problem"];
            404: components["responses"]["Problem"];
            409: components["responses"]["IdempotencyKeyReusedProblem"];
            412: components["responses"]["Problem"];
        };
    };
    updateApiToken: {
        parameters: {
            query?: never;
            header?: {
                "If-Match"?: components["parameters"]["IfMatch"];
                "Idempotency-Key"?: components["parameters"]["IdempotencyKey"];
                /** @description Required when the operation is authorized with the browser session cookie; ignored for bearer authentication. */
                "X-CSRF-Token"?: components["parameters"]["CsrfTokenIfSession"];
            };
            path: {
                api_token_id: components["parameters"]["ApiTokenId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/merge-patch+json": components["schemas"]["ApiTokenPatch"];
            };
        };
        responses: {
            /** @description Updated redacted token metadata */
            200: {
                headers: {
                    ETag: components["headers"]["ETag"];
                    "X-Request-Id": components["headers"]["RequestId"];
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiToken"];
                };
            };
            400: components["responses"]["InvalidRequestProblem"];
            401: components["responses"]["AuthenticationProblem"];
            403: components["responses"]["Problem"];
            404: components["responses"]["Problem"];
            409: components["responses"]["IdempotencyKeyReusedProblem"];
            412: components["responses"]["Problem"];
            422: components["responses"]["ValidationProblem"];
        };
    };
    listMonitors: {
        parameters: {
            query?: {
                limit?: components["parameters"]["Limit"];
                cursor?: components["parameters"]["Cursor"];
                state?: components["schemas"]["MonitorState"];
                kind?: components["schemas"]["MonitorKind"];
                query?: string;
            };
            header?: never;
            path: {
                project_id: components["parameters"]["ProjectId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Page of monitors */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MonitorPage"];
                };
            };
            400: components["responses"]["Problem"];
            401: components["responses"]["Problem"];
            403: components["responses"]["Problem"];
        };
    };
    createMonitor: {
        parameters: {
            query?: never;
            header?: {
                "Idempotency-Key"?: components["parameters"]["IdempotencyKey"];
            };
            path: {
                project_id: components["parameters"]["ProjectId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["MonitorCreate"];
            };
        };
        responses: {
            /** @description Monitor created */
            201: {
                headers: {
                    ETag: components["headers"]["ETag"];
                    Location?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Monitor"];
                };
            };
            400: components["responses"]["Problem"];
            401: components["responses"]["Problem"];
            403: components["responses"]["Problem"];
            409: components["responses"]["Problem"];
            422: components["responses"]["Problem"];
        };
    };
    getMonitor: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                project_id: components["parameters"]["ProjectId"];
                monitor_id: components["parameters"]["MonitorId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Monitor */
            200: {
                headers: {
                    ETag: components["headers"]["ETag"];
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Monitor"];
                };
            };
            404: components["responses"]["Problem"];
        };
    };
    deleteMonitor: {
        parameters: {
            query?: never;
            header?: {
                "If-Match"?: components["parameters"]["IfMatch"];
                "Idempotency-Key"?: components["parameters"]["IdempotencyKey"];
            };
            path: {
                project_id: components["parameters"]["ProjectId"];
                monitor_id: components["parameters"]["MonitorId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deleted */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            404: components["responses"]["Problem"];
            409: components["responses"]["Problem"];
            412: components["responses"]["Problem"];
        };
    };
    updateMonitor: {
        parameters: {
            query?: never;
            header?: {
                "If-Match"?: components["parameters"]["IfMatch"];
                "Idempotency-Key"?: components["parameters"]["IdempotencyKey"];
            };
            path: {
                project_id: components["parameters"]["ProjectId"];
                monitor_id: components["parameters"]["MonitorId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/merge-patch+json": components["schemas"]["MonitorPatch"];
            };
        };
        responses: {
            /** @description Updated monitor */
            200: {
                headers: {
                    ETag: components["headers"]["ETag"];
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Monitor"];
                };
            };
            404: components["responses"]["Problem"];
            409: components["responses"]["Problem"];
            412: components["responses"]["Problem"];
            422: components["responses"]["Problem"];
        };
    };
    runMonitorCheck: {
        parameters: {
            query?: never;
            header?: {
                "Idempotency-Key"?: components["parameters"]["IdempotencyKey"];
            };
            path: {
                project_id: components["parameters"]["ProjectId"];
                monitor_id: components["parameters"]["MonitorId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Check queued */
            202: {
                headers: {
                    Location?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Operation"];
                };
            };
            404: components["responses"]["Problem"];
            409: components["responses"]["Problem"];
        };
    };
    listMonitorObservations: {
        parameters: {
            query?: {
                limit?: components["parameters"]["Limit"];
                cursor?: components["parameters"]["Cursor"];
            };
            header?: never;
            path: {
                project_id: components["parameters"]["ProjectId"];
                monitor_id: components["parameters"]["MonitorId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Observation page */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ObservationPage"];
                };
            };
            404: components["responses"]["Problem"];
        };
    };
    getMonitorUptime: {
        parameters: {
            query: {
                from: string;
                to: string;
                bucket?: "raw" | "minute" | "hour" | "day";
            };
            header?: never;
            path: {
                project_id: components["parameters"]["ProjectId"];
                monitor_id: components["parameters"]["MonitorId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Time-weighted uptime summary */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UptimeSummary"];
                };
            };
            400: components["responses"]["Problem"];
            404: components["responses"]["Problem"];
        };
    };
    listProbes: {
        parameters: {
            query?: {
                limit?: components["parameters"]["Limit"];
                cursor?: components["parameters"]["Cursor"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Probe page */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ProbePage"];
                };
            };
            401: components["responses"]["Problem"];
            403: components["responses"]["Problem"];
        };
    };
    createProbeEnrollment: {
        parameters: {
            query?: never;
            header?: {
                "Idempotency-Key"?: components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ProbeEnrollmentCreate"];
            };
        };
        responses: {
            /** @description One-time enrollment data */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ProbeEnrollment"];
                };
            };
            422: components["responses"]["Problem"];
        };
    };
    validateConfig: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/yaml": string;
                "application/json": Record<string, never>;
            };
        };
        responses: {
            /** @description Validation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ValidationResult"];
                };
            };
            422: components["responses"]["Problem"];
        };
    };
    planConfig: {
        parameters: {
            query?: {
                prune?: boolean;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": Record<string, never>;
            };
        };
        responses: {
            /** @description Declarative plan */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConfigPlan"];
                };
            };
            422: components["responses"]["Problem"];
        };
    };
    applyConfig: {
        parameters: {
            query?: {
                prune?: boolean;
            };
            header?: {
                "Idempotency-Key"?: components["parameters"]["IdempotencyKey"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": Record<string, never>;
            };
        };
        responses: {
            /** @description Apply operation created */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Operation"];
                };
            };
            409: components["responses"]["Problem"];
            422: components["responses"]["Problem"];
        };
    };
    getPublicStatusPage: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                slug: components["schemas"]["Slug"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Redacted public status projection */
            200: {
                headers: {
                    ETag: components["headers"]["ETag"];
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PublicStatusPage"];
                };
            };
            404: components["responses"]["Problem"];
        };
    };
}
