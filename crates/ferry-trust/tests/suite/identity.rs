use ferry_trust::{Error, Fingerprint, Identity};

#[test]
fn generated_identity_survives_a_der_round_trip() {
    let identity = Identity::generate("测试 Mac").unwrap();
    let restored =
        Identity::from_der(identity.cert_der().to_vec(), identity.key_der().to_vec()).unwrap();
    assert_eq!(restored.fingerprint(), identity.fingerprint());
    assert_eq!(identity.fingerprint(), Fingerprint::of(identity.cert_der()));
}

#[test]
fn each_identity_is_unique() {
    let a = Identity::generate("a").unwrap();
    let b = Identity::generate("a").unwrap();
    assert_ne!(a.fingerprint(), b.fingerprint());
}

#[test]
fn a_key_from_another_identity_is_rejected() {
    let a = Identity::generate("a").unwrap();
    let b = Identity::generate("b").unwrap();
    assert!(matches!(
        Identity::from_der(a.cert_der().to_vec(), b.key_der().to_vec()),
        Err(Error::InvalidIdentity(_))
    ));
}

#[test]
fn garbage_is_rejected() {
    let a = Identity::generate("a").unwrap();
    assert!(Identity::from_der(Vec::new(), a.key_der().to_vec()).is_err());
    assert!(Identity::from_der(a.cert_der().to_vec(), vec![1, 2, 3]).is_err());
}

#[test]
fn debug_output_hides_the_private_key() {
    let a = Identity::generate("a").unwrap();
    let text = format!("{a:?}");
    assert!(text.contains(&a.fingerprint().short()));
    assert!(!text.contains("key"), "{text}");
}
