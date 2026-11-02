use ferry_update::{Release, Verdict, Version, judge};

use crate::support::manifest;

fn release() -> Release {
    Release::from_manifest(manifest()).unwrap()
}

#[test]
fn a_newer_version_is_an_update() {
    let r = release();
    assert_eq!(
        judge(&r, &Version::new(0, 1, 0), r.published + 60, None),
        Verdict::Newer
    );
    assert_eq!(
        judge(&r, &Version::new(0, 2, 0), r.published + 60, None),
        Verdict::UpToDate
    );
    assert_eq!(
        judge(&r, &Version::new(0, 3, 0), r.published + 60, None),
        Verdict::UpToDate
    );
}

#[test]
fn an_expired_manifest_cannot_say_up_to_date() {
    let r = release();
    assert_eq!(
        judge(&r, &Version::new(0, 2, 0), r.expires, None),
        Verdict::Expired
    );
}

#[test]
fn a_manifest_older_than_one_seen_before_is_a_rollback() {
    let r = release();
    let now = r.published + 60;
    assert_eq!(
        judge(&r, &Version::new(0, 1, 0), now, Some(r.published + 1)),
        Verdict::Rollback
    );
    assert_eq!(
        judge(&r, &Version::new(0, 1, 0), now, Some(r.published)),
        Verdict::Newer
    );
}
