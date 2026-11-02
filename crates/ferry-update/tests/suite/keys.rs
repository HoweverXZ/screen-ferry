use std::collections::BTreeSet;

use ferry_update::RELEASE_KEYS;

#[test]
fn the_built_in_release_keys_are_two_different_minisign_public_keys() {
    assert_eq!(RELEASE_KEYS.len(), 2, "日常发布用一把，备用一把");
    for key in RELEASE_KEYS {
        assert!(
            minisign::PublicKey::from_base64(key).is_ok(),
            "{key} 不是有效的 minisign 公钥"
        );
    }
    let distinct: BTreeSet<_> = RELEASE_KEYS.iter().collect();
    assert_eq!(distinct.len(), RELEASE_KEYS.len(), "两把不能是同一把");
}
