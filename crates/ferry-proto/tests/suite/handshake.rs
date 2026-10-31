use ferry_proto::{
    Capabilities, Channel, Codec, Hello, Message, PROTOCOL_VERSION, Reject, RejectReason,
};

use crate::support::encode;

#[test]
fn accept_negotiates_common_capabilities() {
    let hello = Hello::new(
        Channel::Input,
        Capabilities::CLIPBOARD | Capabilities::RELATIVE_POINTER,
        0,
        "Mac",
    );
    let welcome = hello.accept(Capabilities::CLIPBOARD, 77, "PC").unwrap();

    assert_eq!(welcome.version, PROTOCOL_VERSION);
    assert_eq!(welcome.capabilities, Capabilities::CLIPBOARD);
    assert_eq!(welcome.session, 77);
    assert_eq!(welcome.name, "PC");
}

#[test]
fn accept_drops_capabilities_it_does_not_know() {
    let from_newer_peer = Capabilities::from_bits_retain(1 << 63) | Capabilities::CLIPBOARD;
    let hello = Hello::new(Channel::Input, from_newer_peer, 0, "Mac");
    let welcome = hello.accept(Capabilities::all(), 1, "PC").unwrap();
    assert_eq!(welcome.capabilities, Capabilities::CLIPBOARD);
}

#[test]
fn accept_rejects_other_versions() {
    let hello = Hello {
        version: PROTOCOL_VERSION + 1,
        ..Hello::new(Channel::Input, Capabilities::all(), 0, "Mac")
    };
    assert_eq!(
        hello.accept(Capabilities::all(), 1, "PC"),
        Err(Reject {
            reason: RejectReason::INCOMPATIBLE_VERSION,
            version: PROTOCOL_VERSION
        }),
    );
}

/// 握手消息的格式与版本无关：任何版本的 Hello 都能解出版本号，才能正确拒绝。
#[test]
fn hello_from_any_version_still_decodes() {
    let future = Message::Hello(Hello {
        version: 999,
        ..Hello::new(Channel::Data, Capabilities::all(), 5, "未来版本")
    });
    let buf = encode(Channel::Data, &future);
    let (decoded, _) = Codec::new(Channel::Data).decode(&buf).unwrap().unwrap();
    assert_eq!(decoded, future);
}
