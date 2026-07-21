#![forbid(unsafe_code)]

use std::{error::Error, net::IpAddr, str::FromStr};

use takt_application::Argon2idConfig;
use takt_application::api_token::{
    ApiTokenHasher, ApiTokenSecret, ApiTokenSecretGenerator, TokenSecretGenerator,
};
use takt_domain::{
    ApiTokenId, OrganizationId, ResourceId, UtcTimestamp,
    api_token::{ApiToken, ApiTokenKind, ApiTokenPrefix, ApiTokenScope, IpNetwork, TokenActor},
};
use uuid::Uuid;

fn resource_id(value: &str) -> Result<ResourceId, Box<dyn Error>> {
    Ok(ResourceId::from_uuid(Uuid::parse_str(value)?)?)
}

#[test]
fn token_actor_grants_only_exact_scopes() -> Result<(), Box<dyn Error>> {
    let actor = TokenActor::new(
        ApiTokenId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000001")?),
        OrganizationId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000002")?),
        None,
        vec![ApiTokenScope::from_str("monitors:read")?],
    )?;

    assert!(actor.allows(&ApiTokenScope::from_str("monitors:read")?));
    assert!(!actor.allows(&ApiTokenScope::from_str("monitors:write")?));
    assert!(!actor.allows(&ApiTokenScope::from_str("checks:execute")?));
    assert!(ApiTokenScope::from_str("monitors:*").is_err());
    assert!(format!("{actor:?}").contains("monitors:read"));
    Ok(())
}

#[test]
fn cidrs_are_canonical_and_enforce_the_source_address() -> Result<(), Box<dyn Error>> {
    let v4 = IpNetwork::from_str("192.0.2.0/24")?;
    let v6 = IpNetwork::from_str("2001:db8::/32")?;
    assert!(v4.contains(IpAddr::from_str("192.0.2.7")?));
    assert!(!v4.contains(IpAddr::from_str("192.0.3.7")?));
    assert!(v6.contains(IpAddr::from_str("2001:db8::7")?));
    assert!(IpNetwork::from_str("192.0.2.7/24").is_err());
    assert!(IpNetwork::from_str("2001:db8::1/32").is_err());
    Ok(())
}

#[test]
fn status_and_ip_restrictions_are_evaluated_without_secrets() -> Result<(), Box<dyn Error>> {
    let token = ApiToken {
        id: ApiTokenId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000011")?),
        organization_id: OrganizationId::from_resource_id(resource_id(
            "019b3cf0-0000-7000-8000-000000000012",
        )?),
        project_id: None,
        name: "read-only".to_owned(),
        kind: ApiTokenKind::Personal,
        token_prefix: ApiTokenPrefix::from_str("takt_0011223344556677")?,
        scopes: vec![ApiTokenScope::from_str("monitors:read")?],
        ip_networks: vec![IpNetwork::from_str("192.0.2.0/24")?],
        expires_at: Some(UtcTimestamp::from_unix_micros(200)),
        last_used_at: None,
        revoked_at: None,
        created_at: UtcTimestamp::from_unix_micros(100),
        updated_at: UtcTimestamp::from_unix_micros(100),
        version: 1,
    };
    assert!(token.authorizes_source(
        UtcTimestamp::from_unix_micros(199),
        IpAddr::from_str("192.0.2.99")?
    ));
    assert!(!token.authorizes_source(
        UtcTimestamp::from_unix_micros(200),
        IpAddr::from_str("192.0.2.99")?
    ));
    assert!(!token.authorizes_source(
        UtcTimestamp::from_unix_micros(199),
        IpAddr::from_str("198.51.100.1")?
    ));
    Ok(())
}

#[test]
fn generated_tokens_and_slow_hashes_are_redacted() -> Result<(), Box<dyn Error>> {
    let generator = ApiTokenSecretGenerator;
    let first = generator.generate()?;
    let second = generator.generate()?;
    assert_ne!(first.expose_once(), second.expose_once());
    assert!(first.expose_once().starts_with(first.lookup_prefix()));
    assert_eq!(first.lookup_prefix().len(), 21);
    assert_eq!(first.secret_entropy_bits(), 256);
    assert_eq!(format!("{first:?}"), "ApiTokenSecret([REDACTED])");

    let hasher = ApiTokenHasher::new(Argon2idConfig::testing());
    let hash = hasher.hash(&first)?;
    assert_eq!(format!("{hash:?}"), "ApiTokenHash([REDACTED])");
    assert!(hash.expose_for_persistence().starts_with("$argon2id$"));
    assert!(!hash.expose_for_persistence().contains(first.expose_once()));
    assert!(hasher.verify(&first, &hash)?);
    let wrong = ApiTokenSecret::from_client_input(second.expose_once().to_owned())?;
    assert!(!hasher.verify(&wrong, &hash)?);
    Ok(())
}
