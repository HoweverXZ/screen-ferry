use ferry_platform_macos::parse_scutil_proxy;

#[test]
fn the_https_proxy_is_preferred_over_the_http_one() {
    let text = "<dictionary> {\n  HTTPEnable : 1\n  HTTPPort : 8080\n  HTTPProxy : 10.0.0.1\n  HTTPSEnable : 1\n  HTTPSPort : 7890\n  HTTPSProxy : 127.0.0.1\n  SOCKSEnable : 1\n}\n";
    assert_eq!(
        parse_scutil_proxy(text).as_deref(),
        Some("http://127.0.0.1:7890")
    );
}

#[test]
fn a_disabled_proxy_is_ignored() {
    let text = "<dictionary> {\n  HTTPSEnable : 0\n  HTTPSPort : 7890\n  HTTPSProxy : 127.0.0.1\n  HTTPEnable : 1\n  HTTPPort : 8080\n  HTTPProxy : proxy.lan\n}\n";
    assert_eq!(
        parse_scutil_proxy(text).as_deref(),
        Some("http://proxy.lan:8080")
    );
    assert_eq!(parse_scutil_proxy("<dictionary> {\n}\n"), None);
}
