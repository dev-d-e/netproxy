use async_trait::async_trait;
use lazy_static::lazy_static;
use log::{debug, error, info, trace};
use std::fmt::Debug;
use std::io::{Error, ErrorKind::InvalidData, ErrorKind::NotConnected, Result};
use std::marker::Send;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::oneshot::Receiver;
use tokio::time::{self, Duration};
use tokio_native_tls::native_tls::{
    Identity, Protocol, TlsAcceptor as NativeAcceptor, TlsConnector as NativeConnector,
};
use tokio_native_tls::{TlsAcceptor, TlsConnector, TlsStream};

const CAPACITY: usize = 8192;

lazy_static! {
    static ref TLS_CONNECTOR: TlsConnector = get_connector();
}

fn get_connector() -> TlsConnector {
    let c = match NativeConnector::new() {
        Ok(c) => c,
        Err(e) => panic!("{}", e),
    };
    TlsConnector::from(c)
}

///the minimum supported TLS protocol version is 1.2
fn get_tls(identity: Identity) -> Result<TlsAcceptor> {
    let mut builder = NativeAcceptor::builder(identity);
    builder.min_protocol_version(Some(Protocol::Tlsv12));
    let nta = builder.build().map_err(|e| Error::new(InvalidData, e))?;
    let acceptor = TlsAcceptor::from(nta);
    Ok(acceptor)
}

#[async_trait]
pub(crate) trait FuncControl {
    fn stop_receiver(&mut self) -> Receiver<u8>;

    async fn reject_ip(&self, ip: IpAddr) -> bool;

    async fn velocity(&self, velocity: u32);
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
    pub(crate) async fn new(str: &String) -> Result<Server> {
        info!("Server bind[{}]", str);
        let listener = TcpListener::bind(str).await?;
        let addr = listener.local_addr()?;
        Ok(Server { listener, addr })
    }

    pub(crate) async fn tcp<T>(&mut self, func: T, mut c: impl FuncControl)
    where
        T: FuncStream,
    {
        let mut rx = c.stop_receiver();
        let mut n: u32 = 0;
        let mut interval = time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                socket = self.listener.accept() => {
                    let socket = match socket {
                        Ok((socket, _)) => socket,
                        Err(e) => {
                            error!("Server(tcp) accept error:{:?}", e);
                            continue;
                        }
                    };
                    trace!("Server(tcp) accept");

                    if let Ok(peer_addr) = socket.peer_addr() {
                        let ip = peer_addr.ip();
                        if c.reject_ip(ip).await {
                            info!("Server(tcp) reject ip {:?}", ip);
                            drop(socket);
                            continue;
                        }
                    }

                    let func = func.clone();
                    tokio::spawn(async move {
                        func.tcp(socket).await;
                    });
                    n += 1;
                },
                Ok(_) =  &mut rx => {
                    info!("Server(tcp) {:?} stop", self.addr.to_string());
                    return;
                }
                _ = interval.tick() => {
                    if n > 0 {
                        c.velocity(n).await;
                        n = 0;
                    }
                }
            }
        }
    }

    pub(crate) async fn tls<T>(&mut self, func: T, identity: Identity, mut c: impl FuncControl)
    where
        T: FuncStream,
    {
        let tls_acceptor = match get_tls(identity) {
            Ok(tls_acceptor) => tls_acceptor,
            Err(e) => {
                error!("Server(tls) can not get \"TlsAcceptor\" error:{:?}", e);
                return;
            }
        };
        let mut rx = c.stop_receiver();
        let mut n: u32 = 0;
        let mut interval = time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                socket = self.listener.accept() => {
                    let socket=match socket{
                        Ok((socket, _)) => socket,
                        Err(e) => {
                            error!("Server(tls) accept error:{:?}", e);
                            continue;
                        }
                    };
                    trace!("Server(tls) accept");

                    if let Ok(peer_addr) = socket.peer_addr() {
                        let ip = peer_addr.ip();
                        if c.reject_ip(ip).await {
                            info!("Server(tls) reject ip {:?}", ip);
                            drop(socket);
                            continue;
                        }
                    }

                    let tls_acceptor = tls_acceptor.clone();
                    let func = func.clone();
                    tokio::spawn(async move {
                        let socket = match tls_acceptor.accept(socket).await {
                            Ok(socket) => socket,
                            Err(e) => {
                                error!("Server(tls) acceptor error:{:?}", e);
                                return;
                            }
                        };
                        func.tls(socket).await;
                    });
                    n += 1;
                }
                Ok(_) = &mut rx => {
                    info!("Server(tls) {:?} stop", self.addr.to_string());
                    return;
                }
                _ = interval.tick() => {
                    if n > 0 {
                        c.velocity(n).await;
                        n = 0;
                    }
                }
            }
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
    pub(crate) async fn new(str: &String) -> Result<UdpServer> {
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
            let mut buf = Vec::<u8>::with_capacity(CAPACITY);
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

pub(crate) async fn connect(str: &String) -> Result<TcpStream> {
    let ts = TcpStream::connect(str).await?;
    debug!(
        "connect[{:?}]-[{:?}]",
        &ts.local_addr()
            .map(|s| s.to_string())
            .unwrap_or("".to_string()),
        &ts.peer_addr()
            .map(|s| s.to_string())
            .unwrap_or("".to_string())
    );
    Ok(ts)
}

pub(crate) async fn connect_tls(str: &String) -> Result<TlsStream<TcpStream>> {
    let socket = connect(&str).await?;
    let mut s = str.clone();
    if let Some(n) = str.find(':') {
        s.truncate(n);
    }

    let c = &TLS_CONNECTOR;
    c.connect(s.as_str(), socket)
        .await
        .map_err(|e| Error::new(NotConnected, e))
}
