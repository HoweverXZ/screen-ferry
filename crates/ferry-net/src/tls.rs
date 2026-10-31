//! TLS 1.3：每条连接都加密，双方都出示设备证书。
//!
//! 证书是自签名的，握手只证明对方持有证书对应的私钥。握手后取出对方证书的指纹，
//! 由上层对照信任列表决定是否接受。

use std::net::SocketAddr;
use std::sync::Arc;

use ferry_proto::Channel;
use ferry_trust::{AnyPeerVerifier, Fingerprint, Identity};
use rustls::crypto::ring::default_provider;
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, ServerConfig};
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};

use crate::error::{Error, Result};
use crate::framed::Framed;

/// ALPN 标识，不是本协议的连接在 TLS 阶段就被拒绝。
const ALPN: &[u8] = b"screenferry";
/// 发给服务端的名字。验证器不看它，只为满足 TLS 格式。
const SERVER_NAME: &str = "screenferry.local";

pub type ServerStream = tokio_rustls::server::TlsStream<TcpStream>;
pub type ClientStream = tokio_rustls::client::TlsStream<TcpStream>;

/// 本机身份对应的 TLS 配置，服务端与客户端两种用途都有。克隆很便宜。
#[derive(Clone)]
pub struct Tls {
    acceptor: TlsAcceptor,
    connector: TlsConnector,
}

impl Tls {
    pub fn new(identity: &Identity) -> Result<Self> {
        let provider = Arc::new(default_provider());
        let verifier = AnyPeerVerifier::new(&provider);

        let mut server = ServerConfig::builder_with_provider(provider.clone())
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_client_cert_verifier(verifier.clone())
            .with_single_cert(identity.cert_chain(), identity.private_key())?;
        server.alpn_protocols = vec![ALPN.to_vec()];

        let mut client = ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_client_auth_cert(identity.cert_chain(), identity.private_key())?;
        client.alpn_protocols = vec![ALPN.to_vec()];

        Ok(Self {
            acceptor: TlsAcceptor::from(Arc::new(server)),
            connector: TlsConnector::from(Arc::new(client)),
        })
    }

    /// 作为接受方完成 TLS 握手。连接类型在随后的 [`accept`](crate::accept) 中按对方的 `Hello` 设定。
    pub async fn accept(&self, stream: TcpStream) -> Result<(Framed<ServerStream>, Fingerprint)> {
        stream.set_nodelay(true)?;
        let stream = self.acceptor.accept(stream).await?;
        let peer = peer_fingerprint(
            stream.get_ref().1.alpn_protocol(),
            stream.get_ref().1.peer_certificates(),
        )?;
        Ok((Framed::new(stream, Channel::Input), peer))
    }

    /// 连接到 `addr` 并完成 TLS 握手。
    pub async fn connect(
        &self,
        addr: SocketAddr,
        channel: Channel,
    ) -> Result<(Framed<ClientStream>, Fingerprint)> {
        let stream = TcpStream::connect(addr).await?;
        // 键鼠事件很小、很急，一律关闭 Nagle 算法。
        stream.set_nodelay(true)?;
        let name = ServerName::try_from(SERVER_NAME).expect("常量是合法的主机名");
        let stream = self.connector.connect(name, stream).await?;
        let peer = peer_fingerprint(
            stream.get_ref().1.alpn_protocol(),
            stream.get_ref().1.peer_certificates(),
        )?;
        Ok((Framed::new(stream, channel), peer))
    }
}

fn peer_fingerprint(
    alpn: Option<&[u8]>,
    certs: Option<&[CertificateDer<'_>]>,
) -> Result<Fingerprint> {
    if alpn != Some(ALPN) {
        return Err(Error::WrongProtocol);
    }
    let cert = certs
        .and_then(<[_]>::first)
        .ok_or(Error::NoPeerCertificate)?;
    Ok(Fingerprint::of(cert))
}
