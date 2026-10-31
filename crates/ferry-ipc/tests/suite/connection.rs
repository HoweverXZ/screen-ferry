use ferry_ipc::{Endpoint, Error, Listener, MAX_LINE, Notice, Request, VERSION, connect};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

async fn listen() -> (Listener, Endpoint) {
    Listener::bind().await.unwrap()
}

#[tokio::test]
async fn listens_on_loopback_with_a_fresh_token() {
    let (listener, endpoint) = listen().await;
    assert!(listener.local_addr().unwrap().ip().is_loopback());
    assert_eq!(endpoint.token.len(), 64);
    assert_eq!(endpoint.version, VERSION);
    let (_, other) = listen().await;
    assert_ne!(endpoint.token, other.token);
}

#[tokio::test]
async fn handshake_then_messages_both_ways() {
    let (listener, endpoint) = listen().await;
    let server = tokio::spawn(async move {
        let mut conn = listener
            .accept()
            .await
            .unwrap()
            .authenticate()
            .await
            .unwrap();
        let request: Request = conn.recv().await.unwrap().unwrap();
        assert_eq!(request, Request::SetSharing { on: false });
        conn.send(&Notice::Toast {
            text: "已暂停共享".into(),
        })
        .await
        .unwrap();
    });

    let (mut reader, mut writer) = connect(&endpoint).await.unwrap().split();
    writer
        .send(&Request::SetSharing { on: false })
        .await
        .unwrap();
    let notice: Notice = reader.recv().await.unwrap().unwrap();
    assert_eq!(
        notice,
        Notice::Toast {
            text: "已暂停共享".into()
        }
    );
    server.await.unwrap();
    assert!(
        reader.recv::<Notice>().await.unwrap().is_none(),
        "对方关闭后读到结尾"
    );
}

#[tokio::test]
async fn a_wrong_token_is_rejected() {
    let (listener, mut endpoint) = listen().await;
    let server = tokio::spawn(async move { listener.accept().await.unwrap().authenticate().await });
    endpoint.token = "00".repeat(32);
    assert!(connect(&endpoint).await.is_err());
    assert!(matches!(server.await.unwrap(), Err(Error::BadToken)));
}

#[tokio::test]
async fn a_different_protocol_version_is_rejected() {
    let (listener, endpoint) = listen().await;
    let server = tokio::spawn(async move { listener.accept().await.unwrap().authenticate().await });
    let mut stream = TcpStream::connect(("127.0.0.1", endpoint.port))
        .await
        .unwrap();
    let hello = format!(
        r#"{{"type":"hello","token":"{}","version":99}}"#,
        endpoint.token
    );
    stream
        .write_all(format!("{hello}\n").as_bytes())
        .await
        .unwrap();
    assert!(matches!(
        server.await.unwrap(),
        Err(Error::Version {
            ours: VERSION,
            theirs: 99
        })
    ));
}

#[tokio::test]
async fn other_messages_before_hello_are_rejected() {
    let (listener, endpoint) = listen().await;
    let server = tokio::spawn(async move { listener.accept().await.unwrap().authenticate().await });
    let mut stream = TcpStream::connect(("127.0.0.1", endpoint.port))
        .await
        .unwrap();
    stream
        .write_all(b"{\"type\":\"clear_log\"}\n")
        .await
        .unwrap();
    assert!(matches!(server.await.unwrap(), Err(Error::Unexpected)));
}

#[tokio::test]
async fn an_endless_line_is_cut_off() {
    let (listener, endpoint) = listen().await;
    let server = tokio::spawn(async move { listener.accept().await.unwrap().authenticate().await });
    let mut stream = TcpStream::connect(("127.0.0.1", endpoint.port))
        .await
        .unwrap();
    let _ = stream.write_all(&vec![b'x'; MAX_LINE + 10]).await;
    assert!(matches!(server.await.unwrap(), Err(Error::LineTooLong)));
}
