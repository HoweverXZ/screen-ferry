use ferry_platform_windows::parse_proxy_server;

#[test]
fn a_single_proxy_server_is_used_for_everything() {
    assert_eq!(
        parse_proxy_server("127.0.0.1:7890").as_deref(),
        Some("http://127.0.0.1:7890")
    );
}

#[test]
fn per_scheme_settings_prefer_https() {
    assert_eq!(
        parse_proxy_server("http=10.0.0.1:8080;https=127.0.0.1:7890;socks=127.0.0.1:7891")
            .as_deref(),
        Some("http://127.0.0.1:7890")
    );
    assert_eq!(
        parse_proxy_server("http=10.0.0.1:8080").as_deref(),
        Some("http://10.0.0.1:8080")
    );
    assert_eq!(parse_proxy_server("socks=127.0.0.1:7891"), None);
    assert_eq!(parse_proxy_server(""), None);
}
