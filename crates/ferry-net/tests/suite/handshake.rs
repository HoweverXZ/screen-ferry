use ferry_net::{Error, Framed, accept, initiate};
use ferry_proto::{Capabilities, Channel, Hello, PROTOCOL_VERSION, RejectReason};
use tokio::io::duplex;

#[tokio::test]
async fn both_sides_agree_on_the_session() {
    let (a, b) = duplex(1024);
    let mut client = Framed::new(a, Channel::Input);
    let mut server = Framed::new(b, Channel::Input);

    let hello = Hello::new(Channel::Input, Capabilities::all(), 0, "Mac");
    let (welcome, accepted) = tokio::join!(
        initiate(&mut client, hello),
        accept(&mut server, Capabilities::CLIPBOARD, 42, "PC"),
    );
    let welcome = welcome.unwrap();
    let (hello, sent) = accepted.unwrap();

    assert_eq!(welcome, sent);
    assert_eq!(welcome.session, 42);
    assert_eq!(welcome.capabilities, Capabilities::CLIPBOARD);
    assert_eq!(welcome.name, "PC");
    assert_eq!(hello.name, "Mac");
}

#[tokio::test]
async fn version_mismatch_is_rejected_on_both_sides() {
    let (a, b) = duplex(1024);
    let mut client = Framed::new(a, Channel::Input);
    let mut server = Framed::new(b, Channel::Input);

    let hello = Hello {
        version: PROTOCOL_VERSION + 1,
        ..Hello::new(Channel::Input, Capabilities::empty(), 0, "新版")
    };
    let (initiated, accepted) = tokio::join!(
        initiate(&mut client, hello),
        accept(&mut server, Capabilities::empty(), 1, "旧版")
    );

    for result in [initiated.map(drop), accepted.map(drop)] {
        match result {
            Err(Error::Rejected(r)) => assert_eq!(r.reason, RejectReason::INCOMPATIBLE_VERSION),
            other => panic!("应被拒绝，实际 {other:?}"),
        }
    }
}
