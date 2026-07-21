#![forbid(unsafe_code)]

use std::{error::Error, net::SocketAddr, sync::Arc};

use async_trait::async_trait;
use takt_api::{
    AuthHttpConfig, AuthHttpError, BrowserAuthenticationHttpPort, HttpAuthentication, HttpLogin,
    HttpSecret,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

const REQUEST_ID: &str = "019b0000-0000-7000-8000-000000000777";

struct RejectAuthentication;

struct AcceptAuthentication;

#[async_trait]
impl BrowserAuthenticationHttpPort for RejectAuthentication {
    async fn login(
        &self,
        _username: &str,
        _password: &str,
        _request_id: &str,
    ) -> Result<HttpLogin, AuthHttpError> {
        Err(AuthHttpError::InvalidCredentials)
    }

    async fn current_session(
        &self,
        _session_token: &str,
    ) -> Result<HttpAuthentication, AuthHttpError> {
        Err(AuthHttpError::Unauthenticated)
    }

    async fn logout(
        &self,
        _session_token: &str,
        _csrf_token: &str,
        _request_id: &str,
    ) -> Result<(), AuthHttpError> {
        Err(AuthHttpError::Unauthenticated)
    }
}

#[async_trait]
impl BrowserAuthenticationHttpPort for AcceptAuthentication {
    async fn login(
        &self,
        _username: &str,
        _password: &str,
        _request_id: &str,
    ) -> Result<HttpLogin, AuthHttpError> {
        Ok(HttpLogin {
            authentication: HttpAuthentication {
                user_id: "019b0000-0000-7000-8000-000000000123".to_owned(),
                username: "contract.admin".to_owned(),
                display_name: "Contract Admin".to_owned(),
                permissions: Vec::new(),
                csrf_token: HttpSecret::new("c".repeat(43))?,
                expires_at_unix_micros: 1_784_445_600_123_456,
                absolute_expires_at_unix_micros: 1_785_050_400_123_456,
            },
            session_token: HttpSecret::new("s".repeat(43))?,
        })
    }

    async fn current_session(
        &self,
        _session_token: &str,
    ) -> Result<HttpAuthentication, AuthHttpError> {
        Err(AuthHttpError::Unauthenticated)
    }

    async fn logout(
        &self,
        _session_token: &str,
        _csrf_token: &str,
        _request_id: &str,
    ) -> Result<(), AuthHttpError> {
        Err(AuthHttpError::Unauthenticated)
    }
}

async fn login_with(
    authentication: Arc<dyn BrowserAuthenticationHttpPort>,
    config: AuthHttpConfig,
    username: &str,
    password: &str,
    extra: &str,
) -> Result<String, Box<dyn Error>> {
    let router = takt_api::router_with_auth(authentication, config);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
    });
    let body = format!(r#"{{"username":"{username}","password":"{password}"{extra}}}"#);
    let mut stream = TcpStream::connect(address).await?;
    let request = format!(
        "POST /api/v1/auth/login HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nX-Request-Id: {REQUEST_ID}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).await?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await?;
    server.abort();
    Ok(String::from_utf8(response)?)
}

async fn login(username: &str, password: &str, extra: &str) -> Result<String, Box<dyn Error>> {
    login_with(
        Arc::new(RejectAuthentication),
        AuthHttpConfig::localhost(),
        username,
        password,
        extra,
    )
    .await
}

// PRD-API-005 / PRD-IAM-001: account failures are identical.
#[tokio::test]
async fn login_failure_is_generic_and_contract_shaped() -> Result<(), Box<dyn Error>> {
    let unknown = login("missing.user", "correct horse battery", "").await?;
    let wrong = login("contract.admin", "wrong horse battery", "").await?;
    assert!(unknown.starts_with("HTTP/1.1 401 Unauthorized\r\n"));
    assert!(unknown.contains("content-type: application/problem+json"));
    assert!(unknown.contains(r#""code":"authentication_failed""#));
    assert_eq!(
        unknown.split_once("\r\n\r\n").map(|part| part.1),
        wrong.split_once("\r\n\r\n").map(|part| part.1)
    );
    let malformed = login(
        "contract.admin",
        "correct horse battery",
        r#","extra":true"#,
    )
    .await?;
    assert!(malformed.starts_with("HTTP/1.1 400 Bad Request\r\n"));
    assert!(malformed.contains(r#""code":"invalid_request""#));
    assert!(!unknown.contains("missing.user") && !unknown.contains("correct horse battery"));
    Ok(())
}

// PRD-API-001 / PRD-IAM-001: non-local deployments always secure the cookie.
#[tokio::test]
async fn production_login_cookie_has_required_security_flags() -> Result<(), Box<dyn Error>> {
    let response = login_with(
        Arc::new(AcceptAuthentication),
        AuthHttpConfig::production(),
        "contract.admin",
        "correct horse battery",
        "",
    )
    .await?;
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    let cookie = response
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("set-cookie:"))
        .ok_or("missing Set-Cookie header")?;
    assert!(cookie.contains("HttpOnly; SameSite=Lax; Path=/; Secure"));
    let body = response
        .split_once("\r\n\r\n")
        .map(|parts| parts.1)
        .ok_or("missing response body")?;
    assert!(!body.contains(&"s".repeat(43)));
    Ok(())
}

// PRD-API-005 / PRD-IAM-001: the eleventh attempt from one peer/account is blocked.
#[tokio::test]
async fn login_rate_limit_is_enforced_with_retry_after() -> Result<(), Box<dyn Error>> {
    let router =
        takt_api::router_with_auth(Arc::new(AcceptAuthentication), AuthHttpConfig::localhost());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
    });

    for attempt in 1..=11 {
        let body = r#"{"username":"contract.admin","password":"correct horse battery"}"#;
        let mut stream = TcpStream::connect(address).await?;
        let request = format!(
            "POST /api/v1/auth/login HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).await?;
        let mut response = String::new();
        stream.read_to_string(&mut response).await?;
        if attempt <= 10 {
            assert!(
                response.starts_with("HTTP/1.1 200"),
                "attempt {attempt}: {response}"
            );
        } else {
            assert!(response.starts_with("HTTP/1.1 429"), "{response}");
            assert!(response.to_ascii_lowercase().contains("retry-after:"));
            assert!(response.contains(r#""code":"rate_limit_exceeded""#));
        }
    }
    server.abort();
    Ok(())
}
