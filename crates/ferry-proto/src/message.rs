//! 消息总表：类型号、所属连接、消息体分发。

use ferry_types::{Display, InputEvent, KeyAction, KeyCode, MouseButton, Point};

use crate::clipboard::{ClipboardOffer, ClipboardRequest};
use crate::error::{DecodeError, EncodeError};
use crate::handshake::{Bye, Channel, Hello, Reject, Welcome};
use crate::limits::MAX_DISPLAYS;
use crate::screen::Enter;
use crate::transfer::{Cancel, Chunk};
use crate::wire::{Body, Reader, Writer};

/// 协议中的一条消息。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    // ── 两种连接通用 ──────────────────────
    Hello(Hello),
    Welcome(Welcome),
    Reject(Reject),
    Bye(Bye),
    /// 心跳。对方须尽快以同一个 `nonce` 回复 `Pong`，发送方据此计算往返时间。
    Ping {
        nonce: u64,
    },
    Pong {
        nonce: u64,
    },

    // ── 输入连接 ──────────────────────────
    /// 发送方的显示器布局。连接建立和布局变化时发送。
    Displays(Vec<Display>),
    Enter(Enter),
    /// 光标离开接收方的屏幕。接收方应释放所有仍按下的键和鼠标按键。
    Leave {
        seq: u32,
    },
    Input(InputEvent),

    // ── 数据连接 ──────────────────────────
    ClipboardOffer(ClipboardOffer),
    ClipboardRequest(ClipboardRequest),
    Chunk(Chunk),
    Cancel(Cancel),
}

/// 消息类型号。高字节是类别：0x00 握手与心跳，0x01 输入，0x02 剪贴板，0x03 传输。
mod ty {
    pub(super) const HELLO: u16 = 0x0001;
    pub(super) const WELCOME: u16 = 0x0002;
    pub(super) const REJECT: u16 = 0x0003;
    pub(super) const BYE: u16 = 0x0004;
    pub(super) const PING: u16 = 0x0005;
    pub(super) const PONG: u16 = 0x0006;

    pub(super) const DISPLAYS: u16 = 0x0100;
    pub(super) const ENTER: u16 = 0x0101;
    pub(super) const LEAVE: u16 = 0x0102;
    pub(super) const KEY: u16 = 0x0110;
    pub(super) const POINTER_MOTION: u16 = 0x0120;
    pub(super) const POINTER_DELTA: u16 = 0x0121;
    pub(super) const BUTTON: u16 = 0x0122;
    pub(super) const WHEEL: u16 = 0x0123;

    pub(super) const CLIPBOARD_OFFER: u16 = 0x0200;
    pub(super) const CLIPBOARD_REQUEST: u16 = 0x0201;

    pub(super) const CHUNK: u16 = 0x0300;
    pub(super) const CANCEL: u16 = 0x0301;
}

/// 类型号为 `ty` 的消息能否出现在 `channel` 上。未知类型返回 `None`。
pub(crate) fn allowed(ty: u16, channel: Channel) -> Option<bool> {
    let input = match ty {
        ty::HELLO | ty::WELCOME | ty::REJECT | ty::BYE | ty::PING | ty::PONG => return Some(true),
        ty::DISPLAYS
        | ty::ENTER
        | ty::LEAVE
        | ty::KEY
        | ty::POINTER_MOTION
        | ty::POINTER_DELTA
        | ty::BUTTON
        | ty::WHEEL => true,
        ty::CLIPBOARD_OFFER | ty::CLIPBOARD_REQUEST | ty::CHUNK | ty::CANCEL => false,
        _ => return None,
    };
    Some(input == (channel == Channel::Input))
}

impl Message {
    pub(crate) fn type_id(&self) -> u16 {
        match self {
            Self::Hello(_) => ty::HELLO,
            Self::Welcome(_) => ty::WELCOME,
            Self::Reject(_) => ty::REJECT,
            Self::Bye(_) => ty::BYE,
            Self::Ping { .. } => ty::PING,
            Self::Pong { .. } => ty::PONG,
            Self::Displays(_) => ty::DISPLAYS,
            Self::Enter(_) => ty::ENTER,
            Self::Leave { .. } => ty::LEAVE,
            Self::Input(event) => match event {
                InputEvent::Key { .. } => ty::KEY,
                InputEvent::PointerMotion { .. } => ty::POINTER_MOTION,
                InputEvent::PointerDelta { .. } => ty::POINTER_DELTA,
                InputEvent::Button { .. } => ty::BUTTON,
                InputEvent::Wheel { .. } => ty::WHEEL,
            },
            Self::ClipboardOffer(_) => ty::CLIPBOARD_OFFER,
            Self::ClipboardRequest(_) => ty::CLIPBOARD_REQUEST,
            Self::Chunk(_) => ty::CHUNK,
            Self::Cancel(_) => ty::CANCEL,
        }
    }

    pub(crate) fn encode_body(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        match self {
            Self::Hello(m) => m.encode(w),
            Self::Welcome(m) => m.encode(w),
            Self::Reject(m) => m.encode(w),
            Self::Bye(m) => m.encode(w),
            Self::Ping { nonce } | Self::Pong { nonce } => {
                w.u64(*nonce);
                Ok(())
            }
            Self::Displays(displays) => w.list(displays, MAX_DISPLAYS, "显示器数量"),
            Self::Enter(m) => m.encode(w),
            Self::Leave { seq } => {
                w.u32(*seq);
                Ok(())
            }
            Self::Input(event) => encode_input(event, w),
            Self::ClipboardOffer(m) => m.encode(w),
            Self::ClipboardRequest(m) => m.encode(w),
            Self::Chunk(m) => m.encode(w),
            Self::Cancel(m) => m.encode(w),
        }
    }

    pub(crate) fn decode_body(ty: u16, r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(match ty {
            ty::HELLO => Self::Hello(Hello::decode(r)?),
            ty::WELCOME => Self::Welcome(Welcome::decode(r)?),
            ty::REJECT => Self::Reject(Reject::decode(r)?),
            ty::BYE => Self::Bye(Bye::decode(r)?),
            ty::PING => Self::Ping { nonce: r.u64()? },
            ty::PONG => Self::Pong { nonce: r.u64()? },
            ty::DISPLAYS => Self::Displays(r.list(MAX_DISPLAYS, "显示器数量")?),
            ty::ENTER => Self::Enter(Enter::decode(r)?),
            ty::LEAVE => Self::Leave { seq: r.u32()? },
            ty::KEY => Self::Input(InputEvent::Key {
                code: KeyCode::decode(r)?,
                action: KeyAction::decode(r)?,
            }),
            ty::POINTER_MOTION => Self::Input(InputEvent::PointerMotion {
                position: Point::decode(r)?,
            }),
            ty::POINTER_DELTA => Self::Input(InputEvent::PointerDelta {
                dx: r.i32()?,
                dy: r.i32()?,
            }),
            ty::BUTTON => Self::Input(InputEvent::Button {
                button: MouseButton::decode(r)?,
                pressed: r.bool()?,
                position: Point::decode(r)?,
            }),
            ty::WHEEL => Self::Input(InputEvent::Wheel {
                dx: r.i32()?,
                dy: r.i32()?,
                position: Point::decode(r)?,
            }),
            ty::CLIPBOARD_OFFER => Self::ClipboardOffer(ClipboardOffer::decode(r)?),
            ty::CLIPBOARD_REQUEST => Self::ClipboardRequest(ClipboardRequest::decode(r)?),
            ty::CHUNK => Self::Chunk(Chunk::decode(r)?),
            ty::CANCEL => Self::Cancel(Cancel::decode(r)?),
            _ => return Err(DecodeError::UnknownMessage(ty)),
        })
    }
}

fn encode_input(event: &InputEvent, w: &mut Writer<'_>) -> Result<(), EncodeError> {
    match event {
        InputEvent::Key { code, action } => {
            code.encode(w)?;
            action.encode(w)
        }
        InputEvent::PointerMotion { position } => position.encode(w),
        InputEvent::PointerDelta { dx, dy } => {
            w.i32(*dx);
            w.i32(*dy);
            Ok(())
        }
        InputEvent::Button {
            button,
            pressed,
            position,
        } => {
            button.encode(w)?;
            w.bool(*pressed);
            position.encode(w)
        }
        InputEvent::Wheel { dx, dy, position } => {
            w.i32(*dx);
            w.i32(*dy);
            position.encode(w)
        }
    }
}
