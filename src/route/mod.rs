mod http;

use crate::core::*;
use crate::state;
use async_trait::async_trait;
use log::{debug, error, trace};
use std::sync::Arc;
use tokio::sync::Mutex;

pub(crate) trait FuncRouteAlg: Send + Sync + 'static {
    fn addr(&mut self, buf: &mut Vec<u8>) -> Option<String>;
}

struct RouteFinder(Arc<Mutex<dyn FuncRouteAlg>>, Protoc);

impl Clone for RouteFinder {
    fn clone(&self) -> Self {
        RouteFinder(Arc::clone(&self.0), self.1.clone())
    }
}

#[async_trait]
impl FuncRemote for RouteFinder {
    async fn get(&mut self, buf: &mut Vec<u8>) -> Option<Remote> {
        let mut lock = self.0.lock().await;
        let addr = lock.addr(buf);
        drop(lock);
        let s = addr?;
        trace!("RouteFinder get:({:?})", s);
        Some(Remote::new(self.1.clone(), s))
    }
}

impl RouteFinder {
    fn new(a: impl FuncRouteAlg, p: Protoc) -> Self {
        RouteFinder(Arc::new(Mutex::new(a)), p)
    }
}

#[derive(Debug)]
pub(crate) struct Transfer {
    pub(crate) server_protoc: Protoc,
    pub(crate) server_addr: String,
    pub(crate) remote_protoc: Protoc,
    pub(crate) remote_addrs: Vec<String>,
    pub(crate) proportion: Vec<usize>,
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
    fn addr(&mut self, _buf: &mut Vec<u8>) -> Option<String> {
        let s = self.0[self.2[self.1]].clone();
        self.1 = (self.1 + 1) % self.2.len();
        Some(s)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RouteR {
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

    fn sum(&self) -> usize {
        self.sum
    }
}

pub(crate) fn start_up(tf: Transfer) {
    debug!("{:?}", tf);
    new_thread_tokiort_block_on(async move {
        match tf.server_protoc {
            Protoc::HTTP => http::http(tf).await,
            Protoc::HTTPPT => http::http_pt(tf).await,
            Protoc::TCP => tcp(tf).await,
            Protoc::TLS => tls(tf).await,
            Protoc::UDP => udp(tf).await,
        }
    });
}

async fn tcp(tf: Transfer) {
    debug!("Server tcp start up");
    let mut server = match Server::new(&tf.server_addr).await {
        Ok(server) => server,
        Err(e) => {
            error!("Server error:{:?}", e);
            return;
        }
    };

    let sc = state::hold(server.addr.to_string()).await;

    let protoc = tf.remote_protoc;
    if protoc == Protoc::TCP || protoc == Protoc::TLS {
        let ra = RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion));
        let pd = Procedure::new(RouteFinder::new(ra, protoc), RouteR::new(), RouteR::new());
        server.tcp(pd, sc).await;
    }
}

async fn tls(tf: Transfer) {
    debug!("Server tls start up");
    let i = match valid_identity().await {
        Some(i) => i,
        None => {
            error!("Server fail");
            return;
        }
    };

    let mut server = match Server::new(&tf.server_addr).await {
        Ok(server) => server,
        Err(e) => {
            error!("Server error:{:?}", e);
            return;
        }
    };

    let sc = state::hold(server.addr.to_string()).await;

    let protoc = tf.remote_protoc;
    if protoc == Protoc::TCP || protoc == Protoc::TLS {
        let ra = RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion));
        let pd = Procedure::new(RouteFinder::new(ra, protoc), RouteR::new(), RouteR::new());
        server.tls(pd, i, sc).await;
    }
}

async fn udp(_tf: Transfer) {}
