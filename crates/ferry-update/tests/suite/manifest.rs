use ferry_update::{Error, Manifest, Release, Version};

use crate::support::{Key, bytes, manifest};

#[test]
fn a_signed_manifest_is_parsed() {
    let key = Key::new();
    let data = bytes(&manifest());
    let release = Release::verify(&data, &key.sign(&data), &[&key.public]).unwrap();
    assert_eq!(release.version, Version::new(0, 2, 0));
    assert_eq!(release.published, 1_795_161_600);
    assert_eq!(release.expires - release.published, 180 * 86_400);
    assert_eq!(release.notes.en, "English UI\nDiagnostics log");
    let asset = release.asset("macos-universal").unwrap();
    assert_eq!(asset.file_name(), "ScreenFerry-0.2.0-macos.zip");
    assert!(release.asset("windows-x86_64").is_none());
}

#[test]
fn any_of_the_built_in_keys_will_do() {
    let (old, new) = (Key::new(), Key::new());
    let data = bytes(&manifest());
    let signature = new.sign(&data);
    assert!(Release::verify(&data, &signature, &[&old.public, &new.public]).is_ok());
}

#[test]
fn a_manifest_signed_by_another_key_is_rejected() {
    let (ours, theirs) = (Key::new(), Key::new());
    let data = bytes(&manifest());
    let err = Release::verify(&data, &theirs.sign(&data), &[&ours.public]).unwrap_err();
    assert!(matches!(err, Error::Untrusted), "{err}");
}

#[test]
fn a_changed_manifest_no_longer_matches_its_signature() {
    let key = Key::new();
    let data = bytes(&manifest());
    let signature = key.sign(&data);
    let mut changed = manifest();
    changed.assets.get_mut("macos-universal").unwrap().url = "https://evil.example/x.zip".into();
    let err = Release::verify(&bytes(&changed), &signature, &[&key.public]).unwrap_err();
    assert!(matches!(err, Error::Untrusted), "{err}");
}

#[test]
fn without_built_in_keys_nothing_is_trusted() {
    let key = Key::new();
    let data = bytes(&manifest());
    let err = Release::verify(&data, &key.sign(&data), &[]).unwrap_err();
    assert!(matches!(err, Error::NoKeys), "{err}");
}

#[test]
fn garbage_signatures_and_unknown_formats_are_errors() {
    let key = Key::new();
    let data = bytes(&manifest());
    assert!(matches!(
        Release::verify(&data, "not a signature", &[&key.public]),
        Err(Error::BadSignature(_))
    ));

    let mut future = manifest();
    future.schema = 2;
    let data = bytes(&future);
    assert!(matches!(
        Release::verify(&data, &key.sign(&data), &[&key.public]),
        Err(Error::UnknownSchema(2))
    ));
}

#[test]
fn a_download_matches_only_with_the_same_size_and_hash() {
    let asset = manifest().assets.remove("macos-universal").unwrap();
    assert!(asset.matches(21_000_000, &"AB".repeat(32)));
    assert!(!asset.matches(21_000_001, &"ab".repeat(32)));
    assert!(!asset.matches(21_000_000, &"cd".repeat(32)));
}

#[test]
fn times_round_trip_through_rfc_3339() {
    assert_eq!(Manifest::format_time(1_795_161_600), "2026-11-20T08:00:00Z");
}
