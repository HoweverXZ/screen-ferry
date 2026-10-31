use ferry_trust::{Error, Fingerprint};

#[test]
fn hex_round_trips() {
    let fp = Fingerprint::of(b"some certificate");
    let hex = fp.to_hex();
    assert_eq!(hex.len(), 64);
    assert_eq!(hex.parse::<Fingerprint>().unwrap(), fp);
}

#[test]
fn uppercase_hex_is_accepted() {
    let fp = Fingerprint::of(b"x");
    assert_eq!(
        fp.to_hex().to_uppercase().parse::<Fingerprint>().unwrap(),
        fp
    );
}

#[test]
fn malformed_hex_is_rejected() {
    for bad in [
        "",
        "abc",
        &"g".repeat(64),
        &"ab".repeat(33),
        &"中".repeat(32),
    ] {
        assert!(
            matches!(bad.parse::<Fingerprint>(), Err(Error::InvalidFingerprint)),
            "{bad:?}"
        );
    }
}

#[test]
fn short_form_groups_the_first_eight_bytes() {
    let fp = Fingerprint::from_bytes([
        0xAB, 0xCD, 0x01, 0x23, 0x45, 0x67, 0x89, 0xEF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]);
    assert_eq!(fp.to_string(), "ABCD-0123-4567-89EF");
}
