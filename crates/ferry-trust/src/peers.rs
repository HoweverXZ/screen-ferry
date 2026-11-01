//! 已信任的设备列表。

use std::net::SocketAddr;

use crate::fingerprint::Fingerprint;

/// 一台配对过的设备。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    /// 配对时对方的设备名，只用于显示。
    pub name: String,
    pub fingerprint: Fingerprint,
    /// 上次连上时对方的地址，局域网里找不到它时用。端口为 0 表示不知道它监听的端口
    /// （本机是服务端、它是来连本机的客户端时只知道它的 IP）。
    pub addr: Option<SocketAddr>,
}

impl Peer {
    /// 一台还不知道地址的设备。
    pub fn new(name: impl Into<String>, fingerprint: Fingerprint) -> Self {
        Self {
            name: name.into(),
            fingerprint,
            addr: None,
        }
    }
}

/// 已信任设备，按配对先后排列；同一指纹只出现一次。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeerList {
    peers: Vec<Peer>,
}

impl PeerList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, fingerprint: &Fingerprint) -> Option<&Peer> {
        self.peers.iter().find(|p| p.fingerprint == *fingerprint)
    }

    pub fn contains(&self, fingerprint: &Fingerprint) -> bool {
        self.get(fingerprint).is_some()
    }

    /// 加入一台设备。已存在时更新名字，新的有地址时也更新地址。
    pub fn add(&mut self, peer: Peer) {
        match self
            .peers
            .iter_mut()
            .find(|p| p.fingerprint == peer.fingerprint)
        {
            Some(existing) => {
                existing.name = peer.name;
                if peer.addr.is_some() {
                    existing.addr = peer.addr;
                }
            }
            None => self.peers.push(peer),
        }
    }

    /// 记下一台设备上次连上时的地址。返回是否有变化；不在列表里的设备不记。
    ///
    /// 已经知道它监听的端口时，新地址的端口为 0（只知道 IP）就沿用原来的端口。
    pub fn set_addr(&mut self, fingerprint: &Fingerprint, addr: SocketAddr) -> bool {
        let Some(peer) = self
            .peers
            .iter_mut()
            .find(|p| p.fingerprint == *fingerprint)
        else {
            return false;
        };
        let addr = match peer.addr {
            Some(old) if addr.port() == 0 && old.ip() == addr.ip() => old,
            _ => addr,
        };
        if peer.addr == Some(addr) {
            return false;
        }
        peer.addr = Some(addr);
        true
    }

    /// 移除一台设备，返回它是否存在过。
    pub fn remove(&mut self, fingerprint: &Fingerprint) -> bool {
        let before = self.peers.len();
        self.peers.retain(|p| p.fingerprint != *fingerprint);
        self.peers.len() != before
    }

    pub fn iter(&self) -> impl Iterator<Item = &Peer> {
        self.peers.iter()
    }

    pub fn len(&self) -> usize {
        self.peers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }
}

impl FromIterator<Peer> for PeerList {
    fn from_iter<I: IntoIterator<Item = Peer>>(iter: I) -> Self {
        let mut list = Self::new();
        iter.into_iter().for_each(|peer| list.add(peer));
        list
    }
}
