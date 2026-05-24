use super::*;
use async_trait::async_trait;
use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OnceCell, mpsc, oneshot};
use tokio::time::{Duration, interval};
use tokio_native_tls::native_tls::{
    Identity, Protocol, TlsAcceptor as NativeAcceptor, TlsConnector as NativeConnector,
};
use tokio_native_tls::{TlsAcceptor, TlsConnector, TlsStream};

const CHANNEL_CAPACITY: usize = 1000;

const CAPACITY: usize = 8192;

static TLS_CONNECTOR: OnceCell<TlsConnector> = OnceCell::const_new();

async fn tls_connector() -> &'static TlsConnector {
    TLS_CONNECTOR
        .get_or_init(|| async { get_connector() })
        .await
}

fn get_connector() -> TlsConnector {
    TlsConnector::from(NativeConnector::new().expect("TlsConnector error"))
}

///the minimum supported TLS protocol version is 1.2
fn get_tls(identity: Identity) -> Option<TlsAcceptor> {
    let mut builder = NativeAcceptor::builder(identity);
    builder.min_protocol_version(Some(Protocol::Tlsv12));
    builder
        .build()
        .map_err(|e| error!("can not get TlsAcceptor: {e}"))
        .map(|n| TlsAcceptor::from(n))
        .ok()
}

pub(crate) async fn get_tls_acceptor() -> Option<TlsAcceptor> {
    if let Some(i) = valid_identity().await {
        get_tls(i)
    } else {
        error!("no valid identity");
        None
    }
}

pub(crate) async fn tls_accept(t: &Arc<TlsAcceptor>, s: TcpStream) -> Option<TlsStream<TcpStream>> {
    t.accept(s)
        .await
        .map_err(|e| error!("tls accept: {e}"))
        .ok()
}

pub(crate) enum ControlInfo {
    Close,
    IpScope(Vec<IpAddr>),
    Ip(IpAddr),
}

pub(crate) enum StateInfo {
    Sum(u32, String),
}

#[async_trait]
pub(crate) trait FuncStream: Clone + Send + Sync + 'static {
    async fn consume(self, socket: TcpStream);
}

#[derive(Getters)]
pub(crate) struct Server {
    listener: TcpListener,
    #[getset(get = "pub(crate)")]
    addr: SocketAddr,
    control_receiver: oneshot::Receiver<ControlInfo>,
    state_sender: mpsc::Sender<StateInfo>,
    ip_scope: HashSet<IpAddr>,
}

impl Server {
    pub(crate) async fn new(
        s: &str,
    ) -> Option<(
        Self,
        oneshot::Sender<ControlInfo>,
        mpsc::Receiver<StateInfo>,
    )> {
        let (control_sender, control_receiver) = oneshot::channel();
        let (state_sender, state_receiver) = mpsc::channel(CHANNEL_CAPACITY);

        Self::with_channel(s, control_receiver, state_sender)
            .await
            .map(|o| (o, control_sender, state_receiver))
    }

    pub(crate) async fn with_channel(
        s: &str,
        control_receiver: oneshot::Receiver<ControlInfo>,
        state_sender: mpsc::Sender<StateInfo>,
    ) -> Option<Self> {
        info!("server bind[{}]", s);
        let listener = TcpListener::bind(s)
            .await
            .map_err(|e| error!("server: {e}"))
            .ok()?;
        listener
            .local_addr()
            .map(|addr| Self {
                listener,
                addr,
                control_receiver,
                state_sender,
                ip_scope: HashSet::new(),
            })
            .map_err(|e| error!("server: {e}"))
            .ok()
    }

    pub(crate) fn set_ip_scope(&mut self, ipscope: &Vec<IpAddr>) {
        self.ip_scope.clear();
        for i in ipscope {
            self.ip_scope.insert(*i);
        }
    }

    fn add_ip_scope(&mut self, i: IpAddr) {
        self.ip_scope.insert(i);
    }

    fn reject_ip(&self, ip: IpAddr) -> bool {
        if self.ip_scope.is_empty() {
            return false;
        }
        if self.ip_scope.contains(&ip) {
            return false;
        }
        true
    }

    pub(crate) async fn accept(&mut self, func: impl FuncStream) {
        let mut n: u32 = 0;
        let mut interval = interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                socket = self.listener.accept() => {
                    let socket = match socket {
                        Ok((socket, _)) => socket,
                        Err(e) => {
                            error!("server accept: {e}");
                            continue;
                        }
                    };
                    trace!("server accept");

                    if let Ok(peer_addr) = socket.peer_addr() {
                        let ip = peer_addr.ip();
                        if self.reject_ip(ip) {
                            info!("server reject ip: {ip}");
                            drop(socket);
                            continue;
                        }
                    }

                    let func = func.clone();
                    tokio::spawn(async move {
                        func.consume(socket).await;
                    });
                    n += 1;
                },
                Ok(c) = &mut self.control_receiver => {
                    match c{
                        ControlInfo::Close => {
                            info!("server {:?} stop", self.addr);
                            return;
                        },
                        ControlInfo::IpScope(o) => {
                            self.set_ip_scope(&o)
                        },
                        ControlInfo::Ip(i) => {
                            self.add_ip_scope(i);
                        },
                    }
                }
                _ = interval.tick() => {
                    if n > 0 {
                        if let Ok(_) = self.state_sender.try_send(StateInfo::Sum(n,now_str())){
                            n = 0;
                        }
                    }
                }
            }
        }
    }
}

async fn connect(str: &str) -> Option<TcpStream> {
    TcpStream::connect(str)
        .await
        .inspect(|ts| {
            let a = ts.local_addr().map(|s| s.to_string()).unwrap_or_default();
            let b = ts.peer_addr().map(|s| s.to_string()).unwrap_or_default();
            debug!("connect[{a}]-[{b}]");
        })
        .map_err(|e| error!("connect: {e}"))
        .ok()
}

async fn connect_tls(s: &str, d: &str) -> Option<TlsStream<TcpStream>> {
    let socket = connect(&s).await?;
    tls_connector()
        .await
        .connect(d, socket)
        .await
        .map_err(|e| error!("connect_tls: {e}"))
        .ok()
}

#[derive(CopyGetters, Setters)]
pub(crate) struct Client {
    remote: Remote,
    #[getset(get_copy = "pub(crate)", set = "pub(crate)")]
    send_count: u16,
}

impl Client {
    pub(crate) fn new(remote: Remote) -> Self {
        Self {
            remote,
            send_count: 0,
        }
    }

    pub(crate) async fn tcp_stream(&mut self) -> Option<BufStream<TcpStream>> {
        let t = self.remote.target();
        let n = std::cmp::max(self.send_count, 1);
        if n > 1 {
            for _ in 1..n {
                let o = connect(t).await.map(|stream| BufStream::new(stream));
                if o.is_some() {
                    return o;
                }
            }
        }
        connect(t).await.map(|stream| BufStream::new(stream))
    }

    pub(crate) async fn tls_stream(&mut self) -> Option<BufStream<TlsStream<TcpStream>>> {
        let t = self.remote.target();
        let h = self.remote.host();
        let n = std::cmp::max(self.send_count, 1);
        if n > 1 {
            for _ in 1..n {
                let o = connect_tls(t, h).await.map(|stream| BufStream::new(stream));
                if o.is_some() {
                    return o;
                }
            }
        }
        connect_tls(t, h).await.map(|stream| BufStream::new(stream))
    }
}
