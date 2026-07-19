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
            /** @default 200 */
            expected_status_min: number;
            /** @default 399 */
            expected_status_max: number;
            /** @default 5 */
            follow_redirects: number;
            /** @default true */
            verify_tls: boolean;
            body_contains?: string;
        };
        TcpCheckSpec: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            type: "tcp";
            host: string;
            port: number;
            send_text?: string;
            expect_prefix?: string;
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
            /** @default 30 */
            warning_days: number;
            /** @default 7 */
            critical_days: number;
        };
        PushCheckSpec: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            type: "push";
            grace_ms: number;
        };
        BrowserCheckSpec: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            type: "browser";
            /** Format: uri */
            start_url: string;
            steps: {
                /** @enum {string} */
                action: "navigate" | "click" | "fill" | "wait" | "assert_text" | "assert_url";
                selector?: string;
                value?: string;
            }[];
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
    };
    responses: {
        /** @description Problem Details response */
        Problem: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["Problem"];
            };
        };
    };
    parameters: {
        ProjectId: components["schemas"]["Uuid"];
        MonitorId: components["schemas"]["Uuid"];
        Limit: number;
        Cursor: string;
        IdempotencyKey: string;
        IfMatch: string;
    };
    requestBodies: never;
    headers: {
        /** @description Quoted resource version */
        ETag: string;
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
