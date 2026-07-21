#![forbid(unsafe_code)]

use takt_application::{
    Argon2idConfig, PasswordHasher, SecureTokenGenerator, TokenDigest, TokenGenerator,
    normalize_local_username,
};

// PRD-IAM-001 / PRD-NFR-010: the local identity boundary is deterministic and
// rejects unsafe values before persistence is attempted.
#[test]
fn prd_iam_001_normalizes_and_validates_local_usernames() {
    assert_eq!(
        normalize_local_username("  Local.Admin  ").as_deref(),
        Ok("local.admin")
    );
    assert!(normalize_local_username("admin@example").is_err());
    assert!(normalize_local_username("").is_err());
}

#[test]
fn prd_iam_001_password_bounds_are_enforced() {
    let hasher = PasswordHasher::new(Argon2idConfig::testing());

    assert!(hasher.hash("short").is_err());
    assert!(hasher.hash(&"x".repeat(1_025)).is_err());
    assert!(hasher.hash("correct horse battery").is_ok());
}

#[test]
fn prd_iam_001_argon2id_hash_never_contains_plaintext() {
    let hasher = PasswordHasher::new(Argon2idConfig::testing());
    let password = "correct horse battery";
    let hash = hasher.hash(password).expect("valid test password hashes");

    assert!(hash.expose_for_persistence().starts_with("$argon2id$"));
    assert!(!hash.expose_for_persistence().contains(password));
    assert!(hasher.verify(password, &hash).expect("hash verifies"));
    assert!(
        !hasher
            .verify("different safe password", &hash)
            .expect("hash verifies")
    );
    assert_eq!(format!("{hash:?}"), "PasswordHash([REDACTED])");
}

#[test]
fn prd_iam_001_token_digests_are_typed_and_redacted() {
    let hex = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let digest = TokenDigest::from_sha256_hex(hex).expect("valid SHA-256 hex");

    assert_eq!(digest.expose_for_persistence(), format!("sha256:{hex}"));
    assert_eq!(format!("{digest:?}"), "TokenDigest([REDACTED])");
    assert!(TokenDigest::from_sha256_hex("short").is_err());
    assert!(TokenDigest::from_sha256_hex(&hex.to_ascii_uppercase()).is_err());
    let generator = SecureTokenGenerator;
    let first = generator.generate().expect("OS randomness is available");
    let second = generator.generate().expect("OS randomness is available");
    assert_eq!(first.expose_to_client().len(), 64);
    assert_ne!(first, second);
    assert_eq!(format!("{first:?}"), "OpaqueToken([REDACTED])");
}
