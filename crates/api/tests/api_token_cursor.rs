#![forbid(unsafe_code)]

use takt_api::{ApiTokenCursorBoundary, ApiTokenCursorFilter, ApiTokenCursorKey};
use uuid::Uuid;

// PRD-API-004 / PRD-API-005: list cursors are opaque, signed and bound to the
// documented stable sort plus every active filter.
#[test]
fn api_token_cursor_is_signed_and_filter_bound() -> Result<(), Box<dyn std::error::Error>> {
    assert!(ApiTokenCursorKey::new([0; 32]).is_err());
    let key = ApiTokenCursorKey::new([0x42; 32])?;
    let boundary = ApiTokenCursorBoundary {
        created_at_unix_micros: 1_753_276_800_000_000,
        id: Uuid::parse_str("019c0000-0000-7000-8000-000000000011")?,
    };
    let filter = ApiTokenCursorFilter {
        project_id: Some("019c0000-0000-7000-8000-000000000002".to_owned()),
        kind: Some("service".to_owned()),
        status: Some("active".to_owned()),
        scope: Some("monitors:read".to_owned()),
    };
    let cursor = key.encode(&boundary, &filter)?;
    assert_eq!(key.decode(&cursor, &filter)?, boundary);
    assert!(cursor.len() <= 2_048);

    let mut changed_filter = filter.clone();
    changed_filter.kind = Some("personal".to_owned());
    assert!(key.decode(&cursor, &changed_filter).is_err());
    assert!(key.decode(&"0".repeat(2_049), &filter).is_err());
    assert!(key.decode("not-hex.not-a-mac", &filter).is_err());

    let mut tampered = cursor.into_bytes();
    let last = tampered.last_mut().ok_or("empty cursor")?;
    *last = if *last == b'0' { b'1' } else { b'0' };
    assert!(key.decode(&String::from_utf8(tampered)?, &filter).is_err());
    assert_eq!(format!("{key:?}"), "ApiTokenCursorKey([REDACTED])");
    assert!(!format!("{key:?}").contains("4242"));

    let invalid_boundary = ApiTokenCursorBoundary {
        created_at_unix_micros: boundary.created_at_unix_micros,
        id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000")?,
    };
    assert!(key.encode(&invalid_boundary, &filter).is_err());
    Ok(())
}
