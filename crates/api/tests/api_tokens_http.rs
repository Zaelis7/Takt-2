#![forbid(unsafe_code)]

use std::{
    error::Error,
    io,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use serde_json::Value;
use takt_api::{
    ApiTokenCreateHttpError, ApiTokenCreateHttpPort, ApiTokenCursorKey, ApiTokenHttpCreate,
    ApiTokenHttpCreateContext, ApiTokenHttpCreated, ApiTokenHttpCredential,
    ApiTokenHttpCredentialKind, ApiTokenHttpKind, ApiTokenHttpListPage, ApiTokenHttpListQuery,
    ApiTokenHttpResource, ApiTokenHttpSecret, ApiTokenHttpStatus, ApiTokenReadHttpError,
    ApiTokenReadHttpPort,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use uuid::Uuid;

const REQUEST_ID: &str = "019c1000-0000-7000-8000-000000000001";
const ORGANIZATION_ID: &str = "019c1000-0000-7000-8000-000000000002";
const PROJECT_ID: &str = "019c1000-0000-7000-8000-000000000003";
const TOKEN_ID: &str = "019c1000-0000-7000-8000-000000000011";
const PAGE_TOKEN_ID: &str = "019c1000-0000-7000-8000-000000000010";
const FORBIDDEN_ID: &str = "019c1000-0000-7000-8000-000000000012";
const MISSING_ID: &str = "019c1000-0000-7000-8000-000000000013";
const INTERNAL_ID: &str = "019c1000-0000-7000-8000-000000000014";
const INVALID_RESOURCE_ID: &str = "019c1000-0000-7000-8000-000000000015";
const NOW: i64 = 1_753_276_800_000_000;

#[derive(Debug)]
struct Call {
    kind: ApiTokenHttpCredentialKind,
    credential_debug: String,
    credential_len: usize,
    source: IpAddr,
    id: Uuid,
}

struct RecordingPort {
    calls: Mutex<Vec<Call>>,
    list_calls: Mutex<Vec<ApiTokenHttpListQuery>>,
    resource: ApiTokenHttpResource,
    page: ApiTokenHttpListPage,
}

#[derive(Debug)]
struct CreateCall {
    kind: ApiTokenHttpCredentialKind,
    context_debug: String,
    credential_debug: String,
    csrf_len: Option<usize>,
    idempotency_key: Option<String>,
    request_hash: [u8; 32],
    source: IpAddr,
    create: ApiTokenHttpCreate,
}

#[derive(Default)]
struct RecordingCreatePort {
    calls: Mutex<Vec<CreateCall>>,
}

impl RecordingCreatePort {
    fn calls(&self) -> Result<std::sync::MutexGuard<'_, Vec<CreateCall>>, ApiTokenCreateHttpError> {
        self.calls
            .lock()
            .map_err(|_| ApiTokenCreateHttpError::Internal)
    }
}

#[async_trait]
impl ApiTokenCreateHttpPort for RecordingCreatePort {
    async fn create(
        &self,
        context: ApiTokenHttpCreateContext,
        create: ApiTokenHttpCreate,
    ) -> Result<ApiTokenHttpCreated, ApiTokenCreateHttpError> {
        self.calls()?.push(CreateCall {
            kind: context.credential().kind(),
            context_debug: format!("{context:?}"),
            credential_debug: format!("{:?}", context.credential()),
            csrf_len: context.csrf_token().map(str::len),
            idempotency_key: context.idempotency_key().map(str::to_owned),
            request_hash: *context.request_hash(),
            source: context.source(),
            create: create.clone(),
        });
        match create.name.as_str() {
            "auth" => Err(ApiTokenCreateHttpError::AuthenticationFailed),
            "csrf" => Err(ApiTokenCreateHttpError::CsrfFailed),
            "permission" => Err(ApiTokenCreateHttpError::PermissionDenied),
            "validation" => Err(ApiTokenCreateHttpError::ValidationFailed),
            "idempotency" => Err(ApiTokenCreateHttpError::IdempotencyKeyReused),
            "internal" => Err(ApiTokenCreateHttpError::Internal),
            "invalid" => {
                let mut output = created_output(&create)?;
                output.api_token.name = "drifted-output".to_owned();
                Ok(output)
            }
            _ => created_output(&create),
        }
    }
}

fn created_output(
    create: &ApiTokenHttpCreate,
) -> Result<ApiTokenHttpCreated, ApiTokenCreateHttpError> {
    let token = format!("takt_{}", "z".repeat(80));
    Ok(ApiTokenHttpCreated {
        api_token: ApiTokenHttpResource {
            id: Uuid::parse_str(TOKEN_ID).map_err(|_| ApiTokenCreateHttpError::Internal)?,
            organization_id: Uuid::parse_str(ORGANIZATION_ID)
                .map_err(|_| ApiTokenCreateHttpError::Internal)?,
            project_id: create.project_id,
            name: create.name.clone(),
            kind: create.kind,
            token_prefix: token[..21].to_owned(),
            scopes: create.scopes.clone(),
            ip_networks: create.ip_networks.clone(),
            status: ApiTokenHttpStatus::Active,
            expires_at_unix_micros: create.expires_at_unix_micros,
            last_used_at_unix_micros: None,
            revoked_at_unix_micros: None,
            created_at_unix_micros: NOW,
            updated_at_unix_micros: NOW,
            version: 1,
        },
        token: ApiTokenHttpSecret::new(token)?,
    })
}

impl RecordingPort {
    fn calls(&self) -> Result<std::sync::MutexGuard<'_, Vec<Call>>, ApiTokenReadHttpError> {
        self.calls
            .lock()
            .map_err(|_| ApiTokenReadHttpError::Internal)
    }
}

#[async_trait]
impl ApiTokenReadHttpPort for RecordingPort {
    async fn list(
        &self,
        credential: ApiTokenHttpCredential,
        source: IpAddr,
        query: ApiTokenHttpListQuery,
        _request_id: &str,
    ) -> Result<ApiTokenHttpListPage, ApiTokenReadHttpError> {
        self.calls()?.push(Call {
            kind: credential.kind(),
            credential_debug: format!("{credential:?}"),
            credential_len: credential.expose_to_port().len(),
            source,
            id: Uuid::nil(),
        });
        self.list_calls
            .lock()
            .map_err(|_| ApiTokenReadHttpError::Internal)?
            .push(query.clone());
        if query.scope.as_deref() == Some("forbidden:read") {
            return Err(ApiTokenReadHttpError::PermissionDenied);
        }
        if query.before.is_some() {
            Ok(ApiTokenHttpListPage {
                items: Vec::new(),
                has_more: false,
            })
        } else {
            Ok(self.page.clone())
        }
    }

    async fn get(
        &self,
        credential: ApiTokenHttpCredential,
        source: IpAddr,
        id: Uuid,
        _request_id: &str,
    ) -> Result<ApiTokenHttpResource, ApiTokenReadHttpError> {
        self.calls()?.push(Call {
            kind: credential.kind(),
            credential_debug: format!("{credential:?}"),
            credential_len: credential.expose_to_port().len(),
            source,
            id,
        });
        if id == parse_uuid(FORBIDDEN_ID)? {
            Err(ApiTokenReadHttpError::PermissionDenied)
        } else if id == parse_uuid(MISSING_ID)? {
            Err(ApiTokenReadHttpError::NotFound)
        } else if id == parse_uuid(INTERNAL_ID)? {
            Err(ApiTokenReadHttpError::Internal)
        } else if id == parse_uuid(INVALID_RESOURCE_ID)? {
            let mut resource = self.resource.clone();
            resource.id = id;
            resource.ip_networks = vec!["192.0.2.0/024".to_owned()];
            Ok(resource)
        } else {
            Ok(self.resource.clone())
        }
    }
}

fn parse_uuid(value: &str) -> Result<Uuid, ApiTokenReadHttpError> {
    Uuid::parse_str(value).map_err(|_| ApiTokenReadHttpError::Internal)
}

fn resource() -> Result<ApiTokenHttpResource, Box<dyn Error>> {
    Ok(ApiTokenHttpResource {
        id: Uuid::parse_str(TOKEN_ID)?,
        organization_id: Uuid::parse_str(ORGANIZATION_ID)?,
        project_id: Some(Uuid::parse_str(PROJECT_ID)?),
        name: "monitor reader".to_owned(),
        kind: ApiTokenHttpKind::Personal,
        token_prefix: "takt_0123456789abcdef".to_owned(),
        scopes: vec!["monitors:read".to_owned()],
        ip_networks: vec!["192.0.2.0/24".to_owned()],
        status: ApiTokenHttpStatus::Active,
        expires_at_unix_micros: None,
        last_used_at_unix_micros: Some(NOW),
        revoked_at_unix_micros: None,
        created_at_unix_micros: NOW,
        updated_at_unix_micros: NOW,
        version: 7,
    })
}

fn page() -> Result<ApiTokenHttpListPage, Box<dyn Error>> {
    let first = resource()?;
    let mut second = first.clone();
    second.id = Uuid::parse_str(PAGE_TOKEN_ID)?;
    second.name = "second monitor reader".to_owned();
    second.token_prefix = "takt_1123456789abcdef".to_owned();
    Ok(ApiTokenHttpListPage {
        items: vec![first, second],
        has_more: true,
    })
}

async fn request(
    address: SocketAddr,
    id: &str,
    headers: &[String],
) -> Result<String, Box<dyn Error>> {
    request_path(address, &format!("/api/v1/api-tokens/{id}"), headers).await
}

async fn request_path(
    address: SocketAddr,
    path: &str,
    headers: &[String],
) -> Result<String, Box<dyn Error>> {
    let mut stream = TcpStream::connect(address).await?;
    let headers = headers
        .iter()
        .map(|value| format!("{value}\r\n"))
        .collect::<String>();
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nX-Request-Id: {REQUEST_ID}\r\n{headers}Connection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await?;
    let mut response = String::new();
    stream.read_to_string(&mut response).await?;
    Ok(response)
}

async fn create_request(
    address: SocketAddr,
    headers: &[String],
    payload: &str,
) -> Result<String, Box<dyn Error>> {
    let mut stream = TcpStream::connect(address).await?;
    let headers = headers
        .iter()
        .map(|value| format!("{value}\r\n"))
        .collect::<String>();
    let request = format!(
        "POST /api/v1/api-tokens HTTP/1.1\r\nHost: {address}\r\nX-Request-Id: {REQUEST_ID}\r\nContent-Type: application/json\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(request.as_bytes()).await?;
    let mut response = String::new();
    stream.read_to_string(&mut response).await?;
    Ok(response)
}

fn body(response: &str) -> Result<&str, io::Error> {
    response
        .split_once("\r\n\r\n")
        .map(|parts| parts.1)
        .ok_or_else(|| io::Error::other("response has no body separator"))
}

fn assert_problem(response: &str, status: &str, code: &str) {
    assert!(response.starts_with(status), "{response}");
    assert!(response.contains("content-type: application/problem+json"));
    assert!(response.contains(&format!(r#""code":"{code}""#)));
    assert!(response.contains(&format!(r#""request_id":"{REQUEST_ID}""#)));
}

// PRD-API-001/005 and PRD-IAM-001/004: exercise Get over a real listener,
// including fail-closed credentials, permission, ETag and secret redaction.
#[tokio::test]
async fn api_token_get_boundary_is_contract_shaped_and_fails_closed() -> Result<(), Box<dyn Error>>
{
    let port = Arc::new(RecordingPort {
        calls: Mutex::new(Vec::new()),
        list_calls: Mutex::new(Vec::new()),
        resource: resource()?,
        page: page()?,
    });
    let router =
        takt_api::router_with_api_token_reads(port.clone(), ApiTokenCursorKey::new([7; 32])?);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
    });
    let bearer = format!("takt_{}", "a".repeat(80));
    let session = "s".repeat(43);

    let missing = request(address, TOKEN_ID, &[]).await?;
    assert_problem(
        &missing,
        "HTTP/1.1 401 Unauthorized",
        "authentication_failed",
    );
    let malformed = request(
        address,
        TOKEN_ID,
        &["Authorization: Bearer invalid".to_owned()],
    )
    .await?;
    assert_eq!(body(&missing)?, body(&malformed)?);
    let ambiguous = request(
        address,
        TOKEN_ID,
        &[
            format!("Authorization: Bearer {bearer}"),
            format!("Cookie: takt_session={session}"),
        ],
    )
    .await?;
    assert_eq!(body(&missing)?, body(&ambiguous)?);
    assert!(port.calls()?.is_empty());

    let fetched = request(
        address,
        TOKEN_ID,
        &[format!("Authorization: Bearer {bearer}")],
    )
    .await?;
    assert!(fetched.starts_with("HTTP/1.1 200 OK\r\n"), "{fetched}");
    assert!(fetched.contains("content-type: application/json"));
    assert!(fetched.to_ascii_lowercase().contains("etag: \"7\""));
    let metadata: Value = serde_json::from_str(body(&fetched)?)?;
    assert_eq!(metadata["id"], TOKEN_ID);
    assert_eq!(metadata["status"], "active");
    assert!(metadata.get("token").is_none() && metadata.get("token_hash").is_none());
    assert!(!fetched.contains(&bearer) && !fetched.contains(&session));

    let session_fetch = request(
        address,
        TOKEN_ID,
        &[format!("Cookie: takt_session={session}")],
    )
    .await?;
    assert!(session_fetch.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(!session_fetch.contains(&session));
    let forbidden = request(
        address,
        FORBIDDEN_ID,
        &[format!("Cookie: takt_session={session}")],
    )
    .await?;
    assert_problem(&forbidden, "HTTP/1.1 403 Forbidden", "permission_denied");
    let not_found = request(
        address,
        MISSING_ID,
        &[format!("Cookie: takt_session={session}")],
    )
    .await?;
    assert_problem(&not_found, "HTTP/1.1 404 Not Found", "not_found");
    let internal = request(
        address,
        INTERNAL_ID,
        &[format!("Cookie: takt_session={session}")],
    )
    .await?;
    assert_problem(
        &internal,
        "HTTP/1.1 500 Internal Server Error",
        "internal_error",
    );
    let invalid_resource = request(
        address,
        INVALID_RESOURCE_ID,
        &[format!("Cookie: takt_session={session}")],
    )
    .await?;
    assert_problem(
        &invalid_resource,
        "HTTP/1.1 500 Internal Server Error",
        "internal_error",
    );

    let calls = port.calls()?;
    assert_eq!(calls.len(), 6);
    assert_eq!(calls[0].kind, ApiTokenHttpCredentialKind::Bearer);
    assert_eq!(calls[1].kind, ApiTokenHttpCredentialKind::Session);
    assert_eq!(
        calls[0].credential_debug,
        "ApiTokenHttpCredential(Bearer, [REDACTED])"
    );
    assert_eq!(
        calls[1].credential_debug,
        "ApiTokenHttpCredential(Session, [REDACTED])"
    );
    assert_eq!(calls[0].credential_len, bearer.len());
    assert_eq!(calls[1].credential_len, session.len());
    assert!(calls.iter().all(|call| call.source.is_loopback()));
    assert_eq!(calls[0].id, Uuid::parse_str(TOKEN_ID)?);
    server.abort();
    Ok(())
}

// PRD-API-001/004/005 and PRD-IAM-001/004: exercise List over a real listener,
// including typed filters, stable cursor pages and fail-closed query handling.
#[tokio::test]
async fn api_token_list_boundary_is_filter_and_cursor_bound() -> Result<(), Box<dyn Error>> {
    let port = Arc::new(RecordingPort {
        calls: Mutex::new(Vec::new()),
        list_calls: Mutex::new(Vec::new()),
        resource: resource()?,
        page: page()?,
    });
    let router =
        takt_api::router_with_api_token_reads(port.clone(), ApiTokenCursorKey::new([7; 32])?);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
    });
    let bearer = format!("takt_{}", "a".repeat(80));
    let authorization = [format!("Authorization: Bearer {bearer}")];
    let filters = format!(
        "limit=2&project_id={PROJECT_ID}&kind=personal&status=active&scope=monitors%3Aread"
    );

    let default_page = request_path(address, "/api/v1/api-tokens", &authorization).await?;
    assert!(default_page.starts_with("HTTP/1.1 200 OK\r\n"));

    let first = request_path(
        address,
        &format!("/api/v1/api-tokens?{filters}"),
        &authorization,
    )
    .await?;
    assert!(first.starts_with("HTTP/1.1 200 OK\r\n"), "{first}");
    assert!(first.contains("content-type: application/json"));
    let first_page: Value = serde_json::from_str(body(&first)?)?;
    assert_eq!(first_page["items"].as_array().map(Vec::len), Some(2));
    assert_eq!(first_page["items"][0]["id"], TOKEN_ID);
    assert_eq!(first_page["items"][1]["id"], PAGE_TOKEN_ID);
    let cursor = first_page["next_cursor"]
        .as_str()
        .ok_or_else(|| io::Error::other("first page has no cursor"))?;
    assert!(!cursor.is_empty() && cursor.len() <= 2_048);
    assert!(
        first_page["items"][0].get("token").is_none()
            && first_page["items"][0].get("token_hash").is_none()
    );
    assert!(!first.contains(&bearer));

    let second = request_path(
        address,
        &format!("/api/v1/api-tokens?{filters}&cursor={cursor}"),
        &authorization,
    )
    .await?;
    assert!(second.starts_with("HTTP/1.1 200 OK\r\n"), "{second}");
    let second_page: Value = serde_json::from_str(body(&second)?)?;
    assert_eq!(second_page["items"].as_array().map(Vec::len), Some(0));
    assert!(second_page["next_cursor"].is_null());

    {
        let list_calls = port
            .list_calls
            .lock()
            .map_err(|_| ApiTokenReadHttpError::Internal)?;
        assert_eq!(list_calls.len(), 3);
        assert_eq!(list_calls[0].limit, 50);
        assert_eq!(list_calls[1].limit, 2);
        assert_eq!(list_calls[1].project_id, Some(Uuid::parse_str(PROJECT_ID)?));
        assert_eq!(list_calls[1].kind, Some(ApiTokenHttpKind::Personal));
        assert_eq!(list_calls[1].status, Some(ApiTokenHttpStatus::Active));
        assert_eq!(list_calls[1].scope.as_deref(), Some("monitors:read"));
        assert!(list_calls[1].before.is_none());
        let boundary = list_calls[2]
            .before
            .as_ref()
            .ok_or_else(|| io::Error::other("decoded cursor boundary missing"))?;
        assert_eq!(boundary.created_at_unix_micros, NOW);
        assert_eq!(boundary.id, Uuid::parse_str(PAGE_TOKEN_ID)?);
    }

    let mut tampered = cursor.as_bytes().to_vec();
    let last = tampered
        .last_mut()
        .ok_or_else(|| io::Error::other("cursor is empty"))?;
    *last = if *last == b'0' { b'1' } else { b'0' };
    let tampered = String::from_utf8(tampered)?;
    let invalid_paths = [
        format!("/api/v1/api-tokens?{filters}&cursor={tampered}"),
        format!(
            "/api/v1/api-tokens?limit=2&project_id={PROJECT_ID}&kind=personal&status=revoked&scope=monitors%3Aread&cursor={cursor}"
        ),
        "/api/v1/api-tokens?limit=2&limit=3".to_owned(),
        "/api/v1/api-tokens?limit=0".to_owned(),
        "/api/v1/api-tokens?kind=administrator".to_owned(),
        "/api/v1/api-tokens?unknown=value".to_owned(),
    ];
    for path in invalid_paths {
        let response = request_path(address, &path, &authorization).await?;
        assert_problem(&response, "HTTP/1.1 400 Bad Request", "invalid_cursor");
        assert!(!response.contains(cursor) && !response.contains(&bearer));
    }
    assert_eq!(
        port.list_calls
            .lock()
            .map_err(|_| ApiTokenReadHttpError::Internal)?
            .len(),
        3
    );

    let forbidden = request_path(
        address,
        "/api/v1/api-tokens?scope=forbidden%3Aread",
        &authorization,
    )
    .await?;
    assert_problem(&forbidden, "HTTP/1.1 403 Forbidden", "permission_denied");
    server.abort();
    Ok(())
}

// PRD-API-001/003/005 and PRD-IAM-001/004/005: exercise Create over a real
// listener, including conditional CSRF, exact replay data and secret boundaries.
#[tokio::test]
async fn api_token_create_boundary_is_idempotent_and_secret_safe() -> Result<(), Box<dyn Error>> {
    let port = Arc::new(RecordingCreatePort::default());
    let router = takt_api::router_with_api_token_create(port.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
    });
    let bearer = format!("takt_{}", "a".repeat(80));
    let session = "s".repeat(43);
    let csrf = "c".repeat(43);
    let payload = format!(
        r#"{{"name":"created writer","kind":"service","scopes":["api_tokens:write"],"project_id":"{PROJECT_ID}","expires_at":"2026-07-24T00:00:00Z","ip_networks":["192.0.2.0/24"]}}"#
    );
    let browser_headers = [
        format!("Cookie: takt_session={session}"),
        format!("X-CSRF-Token: {csrf}"),
        "Idempotency-Key: create-key-0001".to_owned(),
    ];

    let created = create_request(address, &browser_headers, &payload).await?;
    assert!(created.starts_with("HTTP/1.1 201 Created\r\n"));
    assert!(created.contains("content-type: application/json"));
    assert!(created.contains("cache-control: no-store"));
    assert!(created.contains(&format!("location: /api/v1/api-tokens/{TOKEN_ID}")));
    assert!(created.to_ascii_lowercase().contains("etag: \"1\""));
    let created_body: Value = serde_json::from_str(body(&created)?)?;
    let opaque = created_body["token"]
        .as_str()
        .ok_or_else(|| io::Error::other("created response has no token"))?;
    assert_eq!(created_body["api_token"]["id"], TOKEN_ID);
    assert_eq!(created_body["api_token"]["project_id"], PROJECT_ID);
    assert_eq!(created_body["api_token"]["status"], "active");
    assert_eq!(created_body["api_token"]["version"], 1);
    assert!(opaque.starts_with("takt_") && opaque.len() >= 48);
    assert!(
        !created.contains(&bearer)
            && !created.contains(&session)
            && !created.contains(&csrf)
            && !created.contains("create-key-0001")
    );

    let replay = create_request(address, &browser_headers, &payload).await?;
    assert!(body(&created)? == body(&replay)?);
    assert!(replay.contains(&format!("location: /api/v1/api-tokens/{TOKEN_ID}")));
    assert!(replay.to_ascii_lowercase().contains("etag: \"1\""));

    let bearer_headers = [
        format!("Authorization: Bearer {bearer}"),
        "X-CSRF-Token: ignored-for-bearer".to_owned(),
    ];
    let bearer_created = create_request(address, &bearer_headers, &payload).await?;
    assert!(bearer_created.starts_with("HTTP/1.1 201 Created\r\n"));
    assert!(body(&created)? == body(&bearer_created)?);

    let large_scopes = (0..100)
        .map(|index| format!("resource{index:03}{}:read", "x".repeat(50)))
        .collect::<Vec<_>>();
    let large_payload =
        serde_json::json!({"name":"large writer","kind":"service","scopes":large_scopes})
            .to_string();
    assert!(large_payload.len() > 4_096);
    let large = create_request(address, &bearer_headers, &large_payload).await?;
    assert!(large.starts_with("HTTP/1.1 201 Created\r\n"));

    let boundary_failures = [
        (
            Vec::new(),
            payload.clone(),
            "401 Unauthorized",
            "authentication_failed",
        ),
        (
            vec![format!("Cookie: takt_session={session}")],
            payload.clone(),
            "403 Forbidden",
            "csrf_failed",
        ),
        (
            browser_headers.to_vec(),
            r#"{"name":"writer","kind":"service","scopes":["api_tokens:write"],"project_id":null}"#
                .to_owned(),
            "400 Bad Request",
            "invalid_request",
        ),
        (
            browser_headers.to_vec(),
            r#"{"name":"writer","kind":"service","scopes":["api_tokens:write"],"unknown":true}"#
                .to_owned(),
            "400 Bad Request",
            "invalid_request",
        ),
        (
            browser_headers.to_vec(),
            r#"{"name":"writer","kind":"service","scopes":["api_tokens:write","api_tokens:write"]}"#
                .to_owned(),
            "422 Unprocessable Entity",
            "validation_failed",
        ),
        (
            browser_headers.to_vec(),
            r#"{"name":"writer","kind":"service","scopes":["api_tokens:write"],"ip_networks":["192.0.2.1/24"]}"#
                .to_owned(),
            "422 Unprocessable Entity",
            "validation_failed",
        ),
        (
            vec![
                format!("Cookie: takt_session={session}"),
                format!("X-CSRF-Token: {csrf}"),
                "Idempotency-Key: short".to_owned(),
            ],
            payload.clone(),
            "400 Bad Request",
            "invalid_request",
        ),
    ];
    for (headers, payload, status, code) in boundary_failures {
        let response = create_request(address, &headers, &payload).await?;
        assert_problem(&response, &format!("HTTP/1.1 {status}"), code);
        assert!(
            !response.contains(opaque)
                && !response.contains(&bearer)
                && !response.contains(&session)
                && !response.contains(&csrf)
        );
    }
    let mut duplicate_csrf = browser_headers.to_vec();
    duplicate_csrf.push(format!("X-CSRF-Token: {csrf}"));
    let response = create_request(address, &duplicate_csrf, &payload).await?;
    assert_problem(&response, "HTTP/1.1 403 Forbidden", "csrf_failed");
    let mut duplicate_key = browser_headers.to_vec();
    duplicate_key.push("Idempotency-Key: create-key-0002".to_owned());
    let response = create_request(address, &duplicate_key, &payload).await?;
    assert_problem(&response, "HTTP/1.1 400 Bad Request", "invalid_request");

    let port_errors = [
        ("auth", "401 Unauthorized", "authentication_failed"),
        ("csrf", "403 Forbidden", "csrf_failed"),
        ("permission", "403 Forbidden", "permission_denied"),
        (
            "validation",
            "422 Unprocessable Entity",
            "validation_failed",
        ),
        ("idempotency", "409 Conflict", "idempotency_key_reused"),
        ("internal", "500 Internal Server Error", "internal_error"),
        ("invalid", "500 Internal Server Error", "internal_error"),
    ];
    for (name, status, code) in port_errors {
        let payload =
            format!(r#"{{"name":"{name}","kind":"service","scopes":["api_tokens:write"]}}"#);
        let response = create_request(address, &bearer_headers, &payload).await?;
        assert_problem(&response, &format!("HTTP/1.1 {status}"), code);
        assert!(!response.contains(opaque) && !response.contains(&bearer));
    }

    let calls = port.calls()?;
    assert_eq!(calls.len(), 11);
    assert_eq!(calls[0].kind, ApiTokenHttpCredentialKind::Session);
    assert_eq!(calls[0].csrf_len, Some(csrf.len()));
    assert_eq!(calls[0].idempotency_key.as_deref(), Some("create-key-0001"));
    assert_eq!(calls[1].request_hash, calls[0].request_hash);
    assert_eq!(calls[2].kind, ApiTokenHttpCredentialKind::Bearer);
    assert_eq!(calls[2].csrf_len, None);
    assert_eq!(calls[2].idempotency_key, None);
    assert_ne!(calls[3].request_hash, calls[0].request_hash);
    assert_eq!(calls[3].create.scopes.len(), 100);
    assert_eq!(
        calls[0].create.project_id,
        Some(Uuid::parse_str(PROJECT_ID)?)
    );
    assert_eq!(calls[0].create.scopes, vec!["api_tokens:write"]);
    assert!(calls.iter().all(|call| call.source.is_loopback()));
    assert!(calls.iter().all(|call| {
        call.credential_debug.ends_with("[REDACTED])")
            && !call.context_debug.contains(&session)
            && !call.context_debug.contains(&bearer)
            && !call.context_debug.contains(&csrf)
            && !call.context_debug.contains("create-key-0001")
    }));
    server.abort();
    Ok(())
}
