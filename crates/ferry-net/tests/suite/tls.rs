use ferry_net::{Error, Tls, accept, initiate};
use ferry_proto::{Capabilities, Channel, Hello, Message};
use ferry_trust::Identity;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

async fn listener() -> (TcpListener, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
}

#[tokio::test]
async fn both_sides_learn_each_others_fingerprint() {
    let (server_id, client_id) = (
        Identity::generate("Mac").unwrap(),
        Identity::generate("PC").unwrap(),
    );
    let (server_tls, client_tls) = (Tls::new(&server_id).unwrap(), Tls::new(&client_id).unwrap());
    let (listener, addr) = listener().await;

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (mut framed, peer) = server_tls.accept(stream).await.unwrap();
        let (hello, _) = accept(&mut framed, Capabilities::all(), "Mac", |_| Ok(9))
            .await
            .unwrap();
        assert_eq!(
            framed.channel(),
            Channel::Pair,
            "按对方的 Hello 切换连接类型"
        );
        let message = framed.recv().await.unwrap();
        (peer, hello.name, message)
    });

    let (mut framed, server_fp) = client_tls.connect(addr, Channel::Pair).await.unwrap();
    let hello = Hello::new(Channel::Pair, Capabilities::empty(), 0, "PC");
    let welcome = initiate(&mut framed, hello).await.unwrap();
    framed
        .send(&Message::PairResult { accepted: true })
        .await
        .unwrap();

    let (client_fp, name, message) = server.await.unwrap();
    assert_eq!(server_fp, server_id.fingerprint());
    assert_eq!(client_fp, client_id.fingerprint());
    assert_eq!((welcome.session, name.as_str()), (9, "PC"));
    assert_eq!(message, Some(Message::PairResult { accepted: true }));
}

#[tokio::test]
async fn a_plain_tcp_client_is_turned_away() {
    let tls = Tls::new(&Identity::generate("Mac").unwrap()).unwrap();
    let (listener, addr) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        tls.accept(stream).await.map(drop)
    });

    let mut plain = TcpStream::connect(addr).await.unwrap();
    // 一个明文的 Hello 帧头，不是 TLS 记录。
    plain.write_all(&[0x01, 0x00, 0x10, 0, 0, 0]).await.unwrap();
    plain.shutdown().await.unwrap();

    assert!(matches!(server.await.unwrap(), Err(Error::Io(_))));
}
