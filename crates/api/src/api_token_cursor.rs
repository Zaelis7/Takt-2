use std::{error::Error, fmt, sync::Arc};

use hmac::{Hmac, KeyInit, Mac};
use sha2_11::{Digest, Sha256};
use uuid::{Uuid, Version};
use zeroize::Zeroizing;

const VERSION: &str = "v1";
const MAX_LENGTH: usize = 2_048;

#[derive(Clone)]
pub struct ApiTokenCursorKey(Arc<Zeroizing<[u8; 32]>>);

impl ApiTokenCursorKey {
    pub fn new(value: [u8; 32]) -> Result<Self, ApiTokenCursorError> {
        if value == [0; 32] {
            return Err(ApiTokenCursorError);
        }
        Ok(Self(Arc::new(Zeroizing::new(value))))
    }

    pub fn encode(
        &self,
        boundary: &ApiTokenCursorBoundary,
        filter: &ApiTokenCursorFilter,
    ) -> Result<String, ApiTokenCursorError> {
        if boundary.id.get_version() != Some(Version::SortRand) {
            return Err(ApiTokenCursorError);
        }
        let payload = format!(
            "{VERSION}:{}:{}:{}",
            boundary.created_at_unix_micros,
            boundary.id,
            hex(&fingerprint(filter))
        );
        Ok(format!(
            "{}.{}",
            hex(payload.as_bytes()),
            hex(&hmac(self, payload.as_bytes())?)
        ))
    }

    pub fn decode(
        &self,
        cursor: &str,
        filter: &ApiTokenCursorFilter,
    ) -> Result<ApiTokenCursorBoundary, ApiTokenCursorError> {
        if cursor.len() > MAX_LENGTH {
            return Err(ApiTokenCursorError);
        }
        let (payload, supplied_mac) = cursor.split_once('.').ok_or(ApiTokenCursorError)?;
        let payload = unhex(payload)?;
        let supplied_mac = unhex(supplied_mac)?;
        let mut verifier =
            Hmac::<Sha256>::new_from_slice(&self.0[..]).map_err(|_| ApiTokenCursorError)?;
        verifier.update(&payload);
        verifier
            .verify_slice(&supplied_mac)
            .map_err(|_| ApiTokenCursorError)?;
        let payload = std::str::from_utf8(&payload).map_err(|_| ApiTokenCursorError)?;
        let mut fields = payload.split(':');
        let version = fields.next().ok_or(ApiTokenCursorError)?;
        let created_at_unix_micros = fields
            .next()
            .ok_or(ApiTokenCursorError)?
            .parse()
            .map_err(|_| ApiTokenCursorError)?;
        let id = Uuid::parse_str(fields.next().ok_or(ApiTokenCursorError)?)
            .map_err(|_| ApiTokenCursorError)?;
        let encoded_filter = fields.next().ok_or(ApiTokenCursorError)?;
        if fields.next().is_some()
            || version != VERSION
            || id.get_version() != Some(Version::SortRand)
            || encoded_filter != hex(&fingerprint(filter))
        {
            return Err(ApiTokenCursorError);
        }
        Ok(ApiTokenCursorBoundary {
            created_at_unix_micros,
            id,
        })
    }
}

impl fmt::Debug for ApiTokenCursorKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiTokenCursorKey([REDACTED])")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiTokenCursorBoundary {
    pub created_at_unix_micros: i64,
    pub id: Uuid,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ApiTokenCursorFilter {
    pub project_id: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub scope: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApiTokenCursorError;

impl fmt::Display for ApiTokenCursorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("API token cursor is invalid")
    }
}

impl Error for ApiTokenCursorError {}

fn fingerprint(filter: &ApiTokenCursorFilter) -> [u8; 32] {
    let mut digest = Sha256::new();
    for value in [
        Some("created_at:desc,id:desc"),
        filter.project_id.as_deref(),
        filter.kind.as_deref(),
        filter.status.as_deref(),
        filter.scope.as_deref(),
    ] {
        let value = value.unwrap_or_default().as_bytes();
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value);
    }
    digest.finalize().into()
}

fn hmac(key: &ApiTokenCursorKey, message: &[u8]) -> Result<[u8; 32], ApiTokenCursorError> {
    let mut signer = Hmac::<Sha256>::new_from_slice(&key.0[..]).map_err(|_| ApiTokenCursorError)?;
    signer.update(message);
    Ok(signer.finalize().into_bytes().into())
}

fn hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(value: &str) -> Result<Vec<u8>, ApiTokenCursorError> {
    if !value.len().is_multiple_of(2) {
        return Err(ApiTokenCursorError);
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char).to_digit(16).ok_or(ApiTokenCursorError)?;
            let low = (pair[1] as char).to_digit(16).ok_or(ApiTokenCursorError)?;
            Ok(((high << 4) | low) as u8)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_sha256_matches_known_vector() -> Result<(), ApiTokenCursorError> {
        let key = ApiTokenCursorKey::new([0x0b; 32])?;
        assert_eq!(
            hex(&hmac(&key, b"Hi There")?),
            "198a607eb44bfbc69903a0f1cf2bbdc5ba0aa3f3d9ae3c1c7a3b1696a0b68cf7"
        );
        Ok(())
    }
}
