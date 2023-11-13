use async_trait::async_trait;
use log::{debug, error, info, trace};
use std::fmt::Debug;
use std::io::ErrorKind;
use std::marker::Send;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio_native_tls::native_tls::{
    Identity, Protocol, TlsAcceptor as NativeAcceptor, TlsConnector as NativeConnector,
};
use tokio_native_tls::{TlsAcceptor, TlsConnector, TlsStream};

fn get_tls(identity: Identity) -> Option<TlsAcceptor> {
    let nta = match NativeAcceptor::builder(identity)
        .min_protocol_version(Some(Protocol::Tlsv12))
        .build()
    {
        Ok(nta) => nta,
        Err(e) => {
            error!("get \"TlsAcceptor\" error:{:?}", e);
            return None;
        }
    };
    let acceptor = TlsAcceptor::from(nta);
    Some(acceptor)
}

#[async_trait]
pub(crate) trait FuncStream: Clone + Send + Sync + 'static {
    async fn tcp(self, socket: TcpStream);

    async fn tls(self, socket: TlsStream<TcpStream>);
}

#[derive(Debug)]
pub(crate) struct Server {
    listener: TcpListener,
    pub(crate) addr: SocketAddr,
}

impl Server {
    pub(crate) async fn new(str: &String) -> std::io::Result<Server> {
        info!("Server bind[{}]", str);
        let listener = TcpListener::bind(str).await?;
        let addr = listener.local_addr()?;
        Ok(Server { listener, addr })
    }

    pub(crate) async fn tcp<T>(self, func: T)
    where
        T: FuncStream,
    {
        loop {
            let socket = match self.listener.accept().await {
                Ok((socket, _)) => socket,
                Err(e) => {
                    error!("Server(tcp) accept error:{:?}", e);
                    continue;
                }
            };
            trace!("Server(tcp) accept");

            let func = func.clone();
            tokio::spawn(async move {
                func.tcp(socket).await;
            });
        }
    }

    pub(crate) async fn tls<T>(self, func: T, identity: Identity)
    where
        T: FuncStream,
    {
        let tls_acceptor = match get_tls(identity) {
            Some(tls_acceptor) => tls_acceptor,
            None => {
                error!("Server(tls) can not get \"TlsAcceptor\"");
                return;
            }
        };
        loop {
            let socket = match self.listener.accept().await {
                Ok((socket, _)) => socket,
                Err(e) => {
                    error!("Server(tls) accept error:{:?}", e);
                    continue;
                }
            };
            trace!("Server(tls) accept");

            let tls_acceptor = tls_acceptor.clone();
            let func = func.clone();
            tokio::spawn(async move {
                let socket = match tls_acceptor.accept(socket).await {
                    Ok(socket) => socket,
                    Err(e) => {
                        error!("Server(tls) accept error:{:?}", e);
                        return;
                    }
                };
                func.tls(socket).await;
            });
        }
    }
}

#[async_trait]
pub(crate) trait FuncData: Clone + Send + Sync + 'static {
    async fn udp(self, socket: Arc<UdpSocket>, buf: Vec<u8>, addr: SocketAddr);
}

pub(crate) struct UdpServer {
    socket: UdpSocket,
    pub(crate) addr: SocketAddr,
}

impl UdpServer {
    pub(crate) async fn new(str: &String) -> std::io::Result<UdpServer> {
        info!("Server bind({})", str);
        let socket = UdpSocket::bind(str).await?;
        let addr = socket.local_addr()?;
        Ok(UdpServer { socket, addr })
    }

    pub(crate) async fn udp<T>(self, func: T)
    where
        T: FuncData,
    {
        let socket = Arc::new(self.socket);
        loop {
            let mut buf = Vec::<u8>::with_capacity(10240);
            match socket.recv_buf_from(&mut buf).await {
                Ok((n, from)) => {
                    trace!("Server(udp) recv {}", n);
                    if n > 0 {
                        let socket = socket.clone();
                        let func = func.clone();
                        tokio::spawn(async move {
                            func.udp(socket, buf, from).await;
                        });
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        trace!("Server(udp) recv WouldBlock");
                    } else {
                        error!("Server(udp) recv error:{:?}", e);
                    }
                }
            }
        }
    }
}

pub(crate) async fn connect(str: &String) -> std::io::Result<TcpStream> {
    let ts = TcpStream::connect(str).await?;
    debug!(
        "connect[{:?}]-[{:?}]",
        if let Ok(s) = &ts.local_addr() {
            s.to_string()
        } else {
            "".to_string()
        },
        if let Ok(s) = &ts.peer_addr() {
            s.to_string()
        } else {
            "".to_string()
        }
    );
    Ok(ts)
}

pub(crate) async fn connect_tls(str: &String) -> std::io::Result<TlsStream<TcpStream>> {
    let socket = connect(&str).await?;
    let c = NativeConnector::new().map_err(|e| std::io::Error::new(ErrorKind::NotConnected, e))?;
    let c = TlsConnector::from(c);
    let mut s = str.clone();
    if let Some(n) = str.find(':') {
        s.truncate(n);
    }
    c.connect(s.as_str(), socket)
        .await
        .map_err(|e| std::io::Error::new(ErrorKind::NotConnected, e))
}
