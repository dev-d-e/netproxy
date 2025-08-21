mod http;

use crate::core::*;
use crate::state::*;
use async_trait::async_trait;
use getset::CopyGetters;
use log::{debug, trace};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_native_tls::TlsAcceptor;

pub(crate) trait FuncRouteAlg: Send + Sync + 'static {
    fn addr(&mut self) -> (String, String);
}

#[derive(Clone)]
struct RouteFinder(Arc<Mutex<dyn FuncRouteAlg>>, Protoc);

impl RouteFinder {
    fn new(a: impl FuncRouteAlg, p: Protoc) -> Self {
        Self(Arc::new(Mutex::new(a)), p)
    }

    async fn get(&mut self) -> Remote {
        let mut lock = self.0.lock().await;
        let (addr, h) = lock.addr();
        drop(lock);
        trace!("RouteFinder get:({:?})", addr);
        Remote::new(self.1.clone(), addr, h)
    }
}

#[derive(Debug)]
pub(crate) struct RouteInfo {
    server_protoc: Protoc,
    server_addr: String,
    remote_protoc: Protoc,
    remote_addrs: Vec<String>,
    proportion: Vec<usize>,
}

impl RouteInfo {
    pub(crate) fn new(
        server_protoc: Protoc,
        server_addr: String,
        remote_protoc: Protoc,
        remote_addrs: Vec<String>,
        proportion: Vec<usize>,
    ) -> Self {
        Self {
            server_protoc,
            server_addr,
            remote_protoc,
            remote_addrs,
            proportion,
        }
    }
}

fn get_index(mut v: Vec<usize>) -> Vec<usize> {
    let mut p = Vec::<(usize, usize)>::new();
    for i in 0..v.len() {
        p.push((i, v[i]));
    }
    v.clear();
    loop {
        p = p
            .iter()
            .filter(|n| n.1 > 0)
            .map(|n| {
                v.push(n.0);
                (n.0, n.1 - 1)
            })
            .collect();
        if p.is_empty() {
            break;
        }
    }
    debug!("get_index:{:?}", v);
    v
}

struct RouteAlg(Vec<String>, usize, Vec<usize>);

impl FuncRouteAlg for RouteAlg {
    fn addr(&mut self) -> (String, String) {
        let s = self.0[self.2[self.1]].clone();
        self.1 = (self.1 + 1) % self.2.len();
        let h = s.split(':').next().unwrap_or_default().to_string();
        (s, h)
    }
}

#[derive(Clone, Debug, CopyGetters)]
pub(crate) struct RouteR {
    #[getset(get_copy = "pub(crate)")]
    sum: usize,
}

#[async_trait]
impl FuncR for RouteR {
    async fn data(&mut self, buf: &mut Vec<u8>) {
        self.sum += buf.len();
    }

    async fn enddata(&mut self, buf: &mut Vec<u8>) {
        self.sum += buf.len();
    }
}

impl RouteR {
    fn new() -> Self {
        Self { sum: 0 }
    }
}

#[inline]
async fn route<T>(
    server: &mut BufStream<T>,
    route: &mut RouteFinder,
    server_data_func: &mut impl FuncR,
    remote_data_func: &mut impl FuncR,
) where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    server.read().await;
    if server.is_empty() {
        return;
    }

    let remote = route.get().await;
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
pub(self) struct RouteTls<T>
where
    T: FuncR,
{
    route: RouteFinder,
    server_data_func: T,
    remote_data_func: T,
    tls_acceptor: Arc<TlsAcceptor>,
}

#[async_trait]
impl<T> FuncStream for RouteTls<T>
where
    T: FuncR,
{
    async fn consume(mut self, server: TcpStream) {
        trace!("route tls start");

        let mut server = if let Ok(s) = tls_accept(&self.tls_acceptor, server).await {
            BufStream::new(s)
        } else {
            return;
        };

        route(
            &mut server,
            &mut self.route,
            &mut self.server_data_func,
            &mut self.remote_data_func,
        )
        .await;
    }
}

impl<T> RouteTls<T>
where
    T: FuncR,
{
    pub(crate) fn new(
        route: RouteFinder,
        server_data_func: T,
        remote_data_func: T,
        t: TlsAcceptor,
    ) -> Self {
        Self {
            route,
            server_data_func,
            remote_data_func,
            tls_acceptor: Arc::new(t),
        }
    }
}

#[derive(Clone)]
pub(self) struct Route<T>
where
    T: FuncR,
{
    route: RouteFinder,
    server_data_func: T,
    remote_data_func: T,
}

#[async_trait]
impl<T> FuncStream for Route<T>
where
    T: FuncR,
{
    async fn consume(mut self, server: TcpStream) {
        trace!("route start");

        let mut server = BufStream::new(server);

        route(
            &mut server,
            &mut self.route,
            &mut self.server_data_func,
            &mut self.remote_data_func,
        )
        .await;
    }
}

impl<T> Route<T>
where
    T: FuncR,
{
    pub(crate) fn new(route: RouteFinder, server_data_func: T, remote_data_func: T) -> Self {
        Self {
            route,
            server_data_func,
            remote_data_func,
        }
    }
}

pub(crate) fn start_up(r: RouteInfo) {
    debug!("start_up {:?}", r);
    new_thread_tokiort_block_on(async move { server(r).await });
}

#[inline]
async fn server(r: RouteInfo) {
    match r.server_protoc {
        Protoc::HTTP => http::http(r).await,
        Protoc::HTTPPT => http::http_pt(r).await,
        Protoc::TCP => {
            debug!("server tcp start up");
            let ra = RouteAlg(r.remote_addrs, 0, get_index(r.proportion));
            let o = Route::new(
                RouteFinder::new(ra, r.remote_protoc),
                RouteR::new(),
                RouteR::new(),
            );
            server_accept(&r.server_addr, o).await;
        }
        Protoc::TLS => {
            debug!("server tls start up");
            if let Ok(t) = get_tls_acceptor().await {
                let ra = RouteAlg(r.remote_addrs, 0, get_index(r.proportion));
                let o = RouteTls::new(
                    RouteFinder::new(ra, r.remote_protoc),
                    RouteR::new(),
                    RouteR::new(),
                    t,
                );
                server_accept(&r.server_addr, o).await;
            }
        }
        Protoc::UDP => {}
    }
}
