use ferry_trust::Fingerprint;
use ferry_trust::pairing::{code, commit, nonce, verify};

fn fp(seed: u8) -> Fingerprint {
    Fingerprint::from_bytes([seed; 32])
}

#[test]
fn both_sides_compute_the_same_code() {
    let (na, nb) = (nonce().unwrap(), nonce().unwrap());
    assert_eq!(
        code(&fp(1), &fp(2), &na, &nb),
        code(&fp(1), &fp(2), &na, &nb)
    );
}

#[test]
fn a_different_fingerprint_changes_the_code() {
    // 中间人两边的指纹不同。逐个换掉任何一项，配对码都应改变（偶然相同的概率百万分之一）。
    let (na, nb) = ([1; 32], [2; 32]);
    let base = code(&fp(1), &fp(2), &na, &nb);
    assert_ne!(code(&fp(3), &fp(2), &na, &nb), base);
    assert_ne!(code(&fp(1), &fp(3), &na, &nb), base);
    assert_ne!(code(&fp(1), &fp(2), &[3; 32], &nb), base);
    assert_ne!(code(&fp(1), &fp(2), &na, &[3; 32]), base);
    assert_ne!(code(&fp(2), &fp(1), &nb, &na), base, "角色互换也不同");
}

#[test]
fn commitment_only_opens_with_its_own_nonce() {
    let n = nonce().unwrap();
    let c = commit(&n);
    assert!(verify(&c, &n));
    assert!(!verify(&c, &nonce().unwrap()));
    assert_ne!(c, n, "承诺不能直接暴露随机数");
}

#[test]
fn codes_are_six_digits_and_spread_out() {
    let mut seen = std::collections::HashSet::new();
    for i in 0..200u8 {
        let c = code(&fp(i), &fp(0), &[i; 32], &[0; 32]);
        assert!(c.value() < 1_000_000);
        let text = c.to_string();
        assert_eq!(text.len(), 7, "{text}");
        assert_eq!(text.as_bytes()[3], b' ');
        seen.insert(c);
    }
    assert!(seen.len() > 195, "配对码应当几乎不重复");
}

#[test]
fn nonces_are_random() {
    assert_ne!(nonce().unwrap(), nonce().unwrap());
}
