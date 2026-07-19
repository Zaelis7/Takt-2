#![forbid(unsafe_code)]

use takt_probe_protocol::v1::ProtocolVersion;

#[test]
fn prd_mon_007_generated_protocol_types_are_available() {
    let version = ProtocolVersion { major: 1, minor: 0 };

    assert_eq!(version.major, 1);
    assert_eq!(version.minor, 0);
}
