//! 测试共用：每种消息的样例、编码帮助函数、可复现的伪随机数。

use ferry_proto::{
    Bye, ByeReason, Cancel, CancelReason, Capabilities, Channel, Chunk, ClipboardItem,
    ClipboardOffer, ClipboardRequest, Codec, Enter, Hello, Message, PROTOCOL_VERSION, Reject,
    RejectReason, Welcome,
};
use ferry_types::{
    ClipboardFormat, Display, DisplayId, InputEvent, KeyAction, KeyCode, LockKeys, MouseButton,
    Point, Rect, WHEEL_NOTCH,
};

/// 每种消息至少一个样例，以及它所属的连接。
pub fn samples() -> Vec<(Channel, Message)> {
    let at = Point::new(-120, 340);
    let input = |event| (Channel::Input, Message::Input(event));

    vec![
        (
            Channel::Input,
            Message::Hello(Hello::new(
                Channel::Input,
                Capabilities::CLIPBOARD,
                0,
                "书房的 Mac",
            )),
        ),
        (
            Channel::Data,
            Message::Welcome(Welcome {
                version: PROTOCOL_VERSION,
                capabilities: Capabilities::all(),
                session: u64::MAX,
                name: "办公室 PC".into(),
            }),
        ),
        (
            Channel::Input,
            Message::Reject(Reject::new(RejectReason::BUSY)),
        ),
        (
            Channel::Data,
            Message::Bye(Bye {
                reason: ByeReason::TIMEOUT,
            }),
        ),
        (
            Channel::Input,
            Message::Ping {
                nonce: 0x0123_4567_89AB_CDEF,
            },
        ),
        (Channel::Data, Message::Pong { nonce: 7 }),
        (
            Channel::Input,
            Message::Displays(vec![
                Display {
                    id: DisplayId(1),
                    bounds: Rect::new(0, 0, 2560, 1440),
                    scale_percent: 200,
                    primary: true,
                },
                Display {
                    id: DisplayId(2),
                    bounds: Rect::new(-1920, 0, 1920, 1080),
                    scale_percent: 100,
                    primary: false,
                },
            ]),
        ),
        (
            Channel::Input,
            Message::Enter(Enter {
                seq: 42,
                position: at,
                locks: LockKeys::CAPS | LockKeys::NUM,
            }),
        ),
        (Channel::Input, Message::Leave { seq: 42 }),
        input(InputEvent::Key {
            code: KeyCode::LEFT_SHIFT,
            action: KeyAction::Down,
        }),
        input(InputEvent::Key {
            code: KeyCode::new(KeyCode::CONSUMER_PAGE, 0xE9),
            action: KeyAction::Repeat,
        }),
        input(InputEvent::PointerMotion { position: at }),
        input(InputEvent::PointerDelta { dx: -3, dy: 5 }),
        input(InputEvent::Button {
            button: MouseButton::RIGHT,
            pressed: true,
            position: at,
        }),
        input(InputEvent::Wheel {
            dx: 0,
            dy: -WHEEL_NOTCH,
            position: at,
        }),
        (
            Channel::Data,
            Message::ClipboardOffer(ClipboardOffer {
                seq: 9,
                items: vec![
                    ClipboardItem {
                        format: ClipboardFormat::TEXT,
                        size: 12,
                    },
                    ClipboardItem {
                        format: ClipboardFormat::PNG,
                        size: 1 << 20,
                    },
                ],
            }),
        ),
        (
            Channel::Data,
            Message::ClipboardRequest(ClipboardRequest {
                seq: 9,
                format: ClipboardFormat::PNG,
                transfer: 3,
            }),
        ),
        (
            Channel::Data,
            Message::Chunk(Chunk {
                transfer: 3,
                last: true,
                data: b"hello".to_vec(),
            }),
        ),
        (
            Channel::Data,
            Message::Cancel(Cancel {
                transfer: 3,
                reason: CancelReason::STALE,
            }),
        ),
    ]
}

pub fn encode(channel: Channel, message: &Message) -> Vec<u8> {
    let mut buf = Vec::new();
    Codec::new(channel)
        .encode(message, &mut buf)
        .expect("样例消息必须能编码");
    buf
}

/// 手工拼一帧，用于构造非法数据。
pub fn frame(ty: u16, payload: &[u8]) -> Vec<u8> {
    let mut buf = ty.to_le_bytes().to_vec();
    buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(payload);
    buf
}

/// xorshift64 伪随机数：种子固定，失败可以稳定复现。
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}
