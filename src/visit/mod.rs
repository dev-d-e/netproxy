use crate::core::*;
use crate::state::*;
use async_trait::async_trait;
use getset::CopyGetters;
use log::{debug, error, trace, warn};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_native_tls::TlsAcceptor;

#[derive(Clone, Debug)]
struct VisitFinder(Protoc);

impl VisitFinder {
    async fn get(&mut self, buf: &mut Vec<u8>) -> Option<Remote> {
        let mut req = HttpRequest::parse(buf)
            .inspect_err(|e| {
                error!("http request:{:?}", e);
                warn!("no remote");
            })
            .ok()?;

        let host = req.get_host();
        if self.0 == Protoc::HTTP {
            let target = if host.contains(':') {
                host.to_string()
            } else {
                format!("{}:443", host)
            };
            Some(Remote::new(self.0, target, host))
        } else if self.0 == Protoc::HTTPPT {
            let target = if host.contains(':') {
                host.to_string()
            } else {
                format!("{}:80", host)
            };
            Some(Remote::new(self.0, target, host))
        } else {
            warn!("no remote");
            None
        }
    }
}

#[derive(Debug)]
pub(crate) struct VisitInfo {
    server_protoc: Protoc,
    server_addr: String,
    remote_protoc: Protoc,
}

impl VisitInfo {
    pub(crate) fn new(server_protoc: Protoc, server_addr: String, remote_protoc: Protoc) -> Self {
        Self {
            server_protoc,
            server_addr,
            remote_protoc,
        }
    }
}

#[derive(Clone, Debug, CopyGetters)]
pub(crate) struct VisitR {
    #[getset(get_copy = "pub(crate)")]
    sum: usize,
}

#[async_trait]
impl FuncR for VisitR {
    async fn data(&mut self, buf: &mut Vec<u8>) {
        self.sum += buf.len();
    }

    async fn enddata(&mut self, buf: &mut Vec<u8>) {
        self.sum += buf.len();
    }
}

impl VisitR {
    fn new() -> Self {
        Self { sum: 0 }
    }
}

#[inline]
async fn visit<T>(
    server: &mut BufStream<T>,
    v: &mut VisitFinder,
    server_data_func: &mut impl FuncR,
    remote_data_func: &mut impl FuncR,
) where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    server.read().await;
    if server.is_empty() {
        return;
    }

    let remote = if let Some(r) = v.get(server.r_buf_mut()).await {
        r
    } else {
        return;
    };

    let remote_protoc = remote.protoc();
    debug!("remote[{:?}]", remote.target());

    let mut client = Client::new(remote);

    if remote_protoc == Protoc::TCP || remote_protoc == Protoc::HTTPPT {
        trace!("to_tcp");

        if let Ok(mut remote) = client.tcp_stream().await {
            read_loop(server, &mut remote, server_data_func, remote_data_func).await;
        }
    } else {
        trace!("to_tls");

        if let Ok(mut remote) = client.tls_stream().await {
            read_loop(server, &mut remote, server_data_func, remote_data_func).await;
        }
    }
}

#[derive(Clone)]
pub(self) struct VisitTls<T>
where
    T: FuncR,
{
    visit: VisitFinder,
    server_data_func: T,
    remote_data_func: T,
    tls_acceptor: Arc<TlsAcceptor>,
}

#[async_trait]
impl<T> FuncStream for VisitTls<T>
where
    T: FuncR,
{
    async fn consume(mut self, server: TcpStream) {
        trace!("visit tls start");

        let mut server = if let Ok(s) = tls_accept(&self.tls_acceptor, server).await {
            BufStream::new(s)
        } else {
            return;
        };

        visit(
            &mut server,
            &mut self.visit,
            &mut self.server_data_func,
            &mut self.remote_data_func,
        )
        .await;
    }
}

impl<T> VisitTls<T>
where
    T: FuncR,
{
    pub(crate) fn new(
        visit: VisitFinder,
        server_data_func: T,
        remote_data_func: T,
        t: TlsAcceptor,
    ) -> Self {
        Self {
            visit,
            server_data_func,
            remote_data_func,
            tls_acceptor: Arc::new(t),
        }
    }
}

#[derive(Clone)]
pub(self) struct Visit<T>
where
    T: FuncR,
{
    visit: VisitFinder,
    server_data_func: T,
    remote_data_func: T,
}

#[async_trait]
impl<T> FuncStream for Visit<T>
where
    T: FuncR,
{
    async fn consume(mut self, server: TcpStream) {
        trace!("visit start");

        let mut server = BufStream::new(server);

        visit(
            &mut server,
            &mut self.visit,
            &mut self.server_data_func,
            &mut self.remote_data_func,
        )
        .await;
    }
}

impl<T> Visit<T>
where
    T: FuncR,
{
    pub(crate) fn new(visit: VisitFinder, server_data_func: T, remote_data_func: T) -> Self {
        Self {
            visit,
            server_data_func,
            remote_data_func,
        }
    }
}

pub(crate) fn start_up(v: VisitInfo) {
    debug!("start_up {:?}", v);
    new_thread_tokiort_block_on(async move { server(v).await });
}

#[inline]
async fn server(v: VisitInfo) {
    match v.server_protoc {
        Protoc::HTTP => {
            debug!("server tls start up");
            if let Ok(t) = get_tls_acceptor().await {
                let o = VisitTls::new(
                    VisitFinder(v.remote_protoc),
                    VisitR::new(),
                    VisitR::new(),
                    t,
                );
                server_accept(&v.server_addr, o).await;
            }
        }
        Protoc::HTTPPT => {
            debug!("server tcp start up");
            let o = Visit::new(VisitFinder(v.remote_protoc), VisitR::new(), VisitR::new());
            server_accept(&v.server_addr, o).await;
        }
        _ => {}
    }
}
