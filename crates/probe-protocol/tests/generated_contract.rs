#![forbid(unsafe_code)]

use prost::Message;
use takt_probe_protocol::v1::{
    AddressFamily, HttpCheck, ProtocolVersion, ProxyBasicAuth, ProxyOptions, PushCheck,
    SecretValueRef,
};

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

#[test]
fn prd_api_002_network_secrets_use_ephemeral_proto_references() {
    let http = HttpCheck {
        url: "https://example.test".into(),
        proxy: Some(ProxyOptions {
            url: "https://proxy.example.test:8443".into(),
            auth: Some(ProxyBasicAuth {
                username: Some(SecretValueRef {
                    ephemeral_key: "proxy-username".into(),
                }),
                password: Some(SecretValueRef {
                    ephemeral_key: "proxy-password".into(),
                }),
            }),
        }),
        resolver: "tls://dns.example.test:853".into(),
        address_family: AddressFamily::Ipv4 as i32,
        ..HttpCheck::default()
    };

    let decoded = HttpCheck::decode(http.encode_to_vec().as_slice())
        .expect("generated HttpCheck network options must round-trip");
    assert_eq!(decoded.resolver, "tls://dns.example.test:853");
    assert_eq!(decoded.address_family, AddressFamily::Ipv4 as i32);
    let proxy = decoded.proxy.expect("proxy must remain present");
    assert_eq!(proxy.url, "https://proxy.example.test:8443");
    let auth = proxy.auth.expect("proxy auth must remain present");
    assert_eq!(
        auth.password
            .expect("proxy password reference must remain present")
            .ephemeral_key,
        "proxy-password",
    );
}
