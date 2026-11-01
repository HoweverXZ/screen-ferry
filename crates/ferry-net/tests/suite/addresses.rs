use ferry_net::local_addresses;

#[test]
fn only_reachable_ipv4_addresses_are_listed() {
    for ip in local_addresses() {
        assert!(ip.is_ipv4(), "{ip}");
        assert!(!ip.is_loopback(), "{ip}");
        if let std::net::IpAddr::V4(v4) = ip {
            assert!(!v4.is_link_local(), "{ip}");
        }
    }
}
