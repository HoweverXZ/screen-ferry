//! 引擎对外报告的事件：连接与断开、往返延迟、光标进出、剪贴板同步、配对结果。
//!
//! 常驻后台用它们拼出给设置界面看的状态；命令行模式不关心，传 [`Events::none`]。

use std::net::SocketAddr;
use std::time::Duration;

use ferry_trust::Fingerprint;
use ferry_types::ClipboardContent;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

/// 一件发生了的事。
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// 服务端开始等待连接。
    Listening { port: u16, announced: bool },
    /// 客户端开始在局域网里寻找服务端。
    Searching,
    /// 客户端在局域网里看到的服务端变了。
    Found(Vec<FoundServer>),
    /// 客户端正在连接。
    Connecting { addr: SocketAddr },
    /// 与对方建立了会话。
    Connected {
        name: String,
        fingerprint: Fingerprint,
        addr: SocketAddr,
    },
    /// 会话结束。`reason` 为 `None` 表示对方正常断开。
    Disconnected { reason: Option<String> },
    /// 一次往返延迟。
    Rtt(Duration),
    /// 光标是否在本机。`hotkey` 表示是按快捷键回来的。
    Cursor { here: bool, hotkey: bool },
    /// 剪贴板同步完成。`to_peer` 为真表示对方拉走了本机的内容。
    Clipboard { to_peer: bool, summary: ClipSummary },
    /// 配对成功，已记住对方。
    Paired {
        name: String,
        fingerprint: Fingerprint,
    },
    /// 配对没有成功。`by_peer` 表示是对方拒绝的。
    PairingRejected { name: String, by_peer: bool },
}

/// 局域网里发现的一台服务端。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundServer {
    pub name: String,
    pub fingerprint: Fingerprint,
    pub addrs: Vec<SocketAddr>,
}

/// 一份剪贴板内容的摘要，用于显示。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipSummary {
    /// 是纯文本时的字符数。
    pub chars: Option<u32>,
    pub bytes: u64,
    /// 纯文本的开头一段，换行替换为空格。
    pub preview: String,
}

impl ClipSummary {
    /// 预览最多保留的字符数。
    const PREVIEW_CHARS: usize = 80;

    pub fn of(content: &ClipboardContent) -> Self {
        let text = content.as_text();
        Self {
            chars: text.map(|t| u32::try_from(t.chars().count()).unwrap_or(u32::MAX)),
            bytes: content.total_len() as u64,
            preview: text
                .map(|t| {
                    t.chars()
                        .take(Self::PREVIEW_CHARS)
                        .map(|c| if c.is_control() { ' ' } else { c })
                        .collect::<String>()
                        .trim()
                        .to_owned()
                })
                .unwrap_or_default(),
        }
    }
}

/// 事件的去处。克隆后共用同一个接收方；没有接收方时事件直接丢弃。
#[derive(Clone, Debug, Default)]
pub struct Events(Option<UnboundedSender<Event>>);

impl Events {
    /// 不需要事件（命令行模式）。
    pub fn none() -> Self {
        Self(None)
    }

    /// 新建一对发送与接收端。
    pub fn channel() -> (Self, UnboundedReceiver<Event>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self(Some(tx)), rx)
    }

    pub fn emit(&self, event: Event) {
        if let Some(tx) = &self.0 {
            let _ = tx.send(event);
        }
    }
}
