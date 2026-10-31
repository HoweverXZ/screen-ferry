//! 已信任的设备列表。

use crate::fingerprint::Fingerprint;

/// 一台配对过的设备。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    /// 配对时对方的设备名，只用于显示。
    pub name: String,
    pub fingerprint: Fingerprint,
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

    /// 加入一台设备。已存在时只更新名字。
    pub fn add(&mut self, peer: Peer) {
        match self
            .peers
            .iter_mut()
            .find(|p| p.fingerprint == peer.fingerprint)
        {
            Some(existing) => existing.name = peer.name,
            None => self.peers.push(peer),
        }
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
