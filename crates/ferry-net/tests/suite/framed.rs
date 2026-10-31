use ferry_net::{Error, Framed};
use ferry_proto::{Channel, Codec, Message};
use tokio::io::{AsyncWriteExt, duplex};

#[tokio::test]
async fn messages_survive_a_byte_at_a_time_stream() {
    let (a, b) = duplex(1);
    let mut tx = Framed::new(a, Channel::Input);
    let mut rx = Framed::new(b, Channel::Input);

    let sent = [
        Message::Ping { nonce: 1 },
        Message::Leave { seq: 7 },
        Message::Pong { nonce: 2 },
    ];
    let send = async {
        for m in &sent {
            tx.send(m).await.unwrap();
        }
        drop(tx);
    };
    let receive = async {
        let mut got = Vec::new();
        while let Some(m) = rx.recv().await.unwrap() {
            got.push(m);
        }
        got
    };
    let ((), got) = tokio::join!(send, receive);
    assert_eq!(got, sent);
}

#[tokio::test]
async fn closing_mid_frame_is_an_error() {
    let (mut a, b) = duplex(64);
    let mut frame = Vec::new();
    Codec::new(Channel::Input)
        .encode(&Message::Ping { nonce: 9 }, &mut frame)
        .unwrap();
    a.write_all(&frame[..frame.len() - 1]).await.unwrap();
    drop(a);

    let mut rx = Framed::new(b, Channel::Input);
    assert!(matches!(rx.recv().await, Err(Error::TruncatedFrame)));
}

#[tokio::test]
async fn protocol_violations_surface_as_decode_errors() {
    let (mut a, b) = duplex(64);
    a.write_all(&[0x77, 0x77, 0, 0, 0, 0]).await.unwrap();

    let mut rx = Framed::new(b, Channel::Input);
    assert!(matches!(rx.recv().await, Err(Error::Decode(_))));
}
