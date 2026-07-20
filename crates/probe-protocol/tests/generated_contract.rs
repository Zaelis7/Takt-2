#![forbid(unsafe_code)]

use prost::Message;
use takt_probe_protocol::v1::{HttpCheck, ProtocolVersion, PushCheck};

#[test]
fn prd_mon_007_generated_protocol_types_are_available() {
    let version = ProtocolVersion { major: 1, minor: 0 };

    assert_eq!(version.major, 1);
    assert_eq!(version.minor, 0);
}

#[test]
fn prd_mon_002_proto_preserves_explicit_zero_and_false_options() {
    let http = HttpCheck {
        url: "https://example.test".into(),
        follow_redirects: Some(0),
        verify_tls: Some(false),
        ..HttpCheck::default()
    };
    let decoded = HttpCheck::decode(http.encode_to_vec().as_slice())
        .expect("generated HttpCheck must round-trip");

    assert_eq!(decoded.follow_redirects, Some(0));
    assert_eq!(decoded.verify_tls, Some(false));

    let push = PushCheck {
        grace_ms: Some(0),
        allow_get: false,
    };
    let decoded = PushCheck::decode(push.encode_to_vec().as_slice())
        .expect("generated PushCheck must round-trip");
    assert_eq!(decoded.grace_ms, Some(0));
}
