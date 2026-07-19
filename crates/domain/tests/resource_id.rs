#![forbid(unsafe_code)]

use std::error::Error;

use takt_domain::ResourceId;

// PRD-NFR-010: domain behavior is deterministic and has no clock or I/O dependency.
#[test]
fn prd_nfr_010_resource_id_parsing_is_deterministic() -> Result<(), Box<dyn Error>> {
    const UUID_V7: &str = "019b0000-0000-7000-8000-000000000001";

    let first = ResourceId::parse(UUID_V7)?;
    let second = ResourceId::parse(UUID_V7)?;

    assert_eq!(first, second);
    assert_eq!(first.to_string(), UUID_V7);
    Ok(())
}

#[test]
fn prd_nfr_010_resource_id_rejects_invalid_input() {
    assert!(ResourceId::parse("not-a-uuid").is_err());
    assert!(
        ResourceId::parse("550e8400-e29b-41d4-a716-446655440000").is_err(),
        "non-v7 UUIDs must not cross the domain boundary"
    );
}
