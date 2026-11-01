//! 本机的局域网地址：显示在设置界面里，方便在另一台电脑上按地址连接。

use std::net::{IpAddr, Ipv4Addr};

/// 本机的局域网 IPv4 地址，常见的家用、办公网段排在前面。
/// 去掉回环、自动分配的链路本地地址（169.254.x.x）和虚拟网卡上的地址；读不到时为空。
pub fn local_addresses() -> Vec<IpAddr> {
    let Ok(interfaces) = if_addrs::get_if_addrs() else {
        return Vec::new();
    };
    let mut found: Vec<Ipv4Addr> = interfaces
        .iter()
        .filter(|i| !i.is_loopback() && !is_virtual(&i.name))
        .filter_map(|i| match i.ip() {
            IpAddr::V4(v4) if !v4.is_link_local() && !v4.is_unspecified() => Some(v4),
            _ => None,
        })
        .collect();
    found.sort_by_key(|ip| (rank(*ip), *ip));
    found.dedup();
    found.into_iter().map(IpAddr::V4).collect()
}

/// 192.168.x.x 最常见，其次 10.x.x.x、172.16–31.x.x，其余排在后面。
fn rank(ip: Ipv4Addr) -> u8 {
    match ip.octets() {
        [192, 168, ..] => 0,
        [10, ..] => 1,
        [172, b, ..] if (16..=31).contains(&b) => 2,
        _ => 3,
    }
}

/// 虚拟机、容器、隧道的网卡：别的电脑连不到这些地址。
fn is_virtual(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "utun",
        "awdl",
        "llw",
        "bridge",
        "vmnet",
        "vboxnet",
        "docker",
        "veth",
        "virbr",
        "vethernet",
        "zt",
        "tailscale",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
}
