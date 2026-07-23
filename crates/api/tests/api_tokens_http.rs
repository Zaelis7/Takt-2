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
    ApiTokenCursorKey, ApiTokenHttpCredential, ApiTokenHttpCredentialKind, ApiTokenHttpKind,
    ApiTokenHttpListPage, ApiTokenHttpListQuery, ApiTokenHttpResource, ApiTokenHttpStatus,
    ApiTokenReadHttpError, ApiTokenReadHttpPort,
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
