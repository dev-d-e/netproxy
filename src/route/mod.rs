mod http;

use crate::core::*;
use crate::state;
use crate::transfer_data;
use async_trait::async_trait;
use log::{debug, error, trace};
use std::sync::Arc;
use tokio::sync::Mutex;

pub(crate) trait FuncRouteAlg: Send + Sync + 'static {
    fn addr(&mut self, buf: &mut Vec<u8>) -> Option<String>;
}

#[macro_export]
macro_rules! route_alg {
    ($name:ident, $self:ident, $buf:ident, $func:stmt, $($ty:ty),*) => {
        struct $name($($ty),*);

        impl FuncRouteAlg for $name {
            fn addr(&mut $self, $buf: &mut Vec<u8>) -> Option<String> {
                $func
            }
        }
    };
}

#[derive(Debug)]
struct RouteFinder<T: FuncRouteAlg>(Arc<Mutex<T>>, Protoc);

impl<T: FuncRouteAlg> Clone for RouteFinder<T> {
    fn clone(&self) -> Self {
        RouteFinder(Arc::clone(&self.0), self.1.clone())
    }
}

#[async_trait]
impl<T: FuncRouteAlg> FuncRemote for RouteFinder<T> {
    async fn get(&self, buf: &mut Vec<u8>) -> Option<(Protoc, String, u32)> {
        let mut lock = self.0.lock().await;
        let addr = lock.addr(buf);
        drop(lock);
        let s = addr?;
        trace!("RouteFinder get:({:?})", s);
        Some((self.1.clone(), s, 0))
    }
}

impl<T: FuncRouteAlg> RouteFinder<T> {
    fn new(a: T, p: Protoc) -> Self {
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

route_alg!(
    RouteAlg,
    self,
    _buf,
    {
        let s = self.0[self.2[self.1]].clone();
        self.1 = (self.1 + 1) % self.2.len();
        Some(s)
    },
    Vec<String>,
    usize,
    Vec<usize>
);

transfer_data!(Empty, self, _buf, {}, {});

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
        let pd = Procedure::new(RouteFinder::new(ra, protoc), Empty, Empty);
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
        let pd = Procedure::new(RouteFinder::new(ra, protoc), Empty, Empty);
        server.tls(pd, i, sc).await;
    }
}

async fn udp(_tf: Transfer) {}
