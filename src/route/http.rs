use super::*;
use crate::{route_alg, transfer_data};
use async_trait::async_trait;
use log::{debug, error, trace};

route_alg!(
    RouteAlg,
    self,
    buf,
    {
        let _req = match parse_request(buf) {
            Ok(req) => req,
            Err(e) => {
                error!("http request error:{:?}", e);
                return None;
            }
        };

        let s = self.0[self.2[self.1]].clone();
        self.1 = (self.1 + 1) % self.2.len();
        Some(s)
    },
    Vec<String>,
    usize,
    Vec<usize>
);

transfer_data!(Empty, self, _buf, {}, {});

async fn http_to_http(tf: Transfer) {
    trace!("http_to_http");
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

    let (tx, rx) = oneshot::channel();
    state::hold(server.addr.to_string(), tx).await;

    let ra = RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion));
    let pd = Procedure::new(RouteFinder::new(ra, Protoc::TLS), Empty, Empty);
    server.tls(pd, i, rx).await;
}

async fn http_to_http_pt(tf: Transfer) {
    trace!("http_to_http_pt");
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

    let (tx, rx) = oneshot::channel();
    state::hold(server.addr.to_string(), tx).await;

    let ra = RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion));
    let pd = Procedure::new(RouteFinder::new(ra, Protoc::TCP), Empty, Empty);
    server.tls(pd, i, rx).await;
}

async fn http_pt_to_http(tf: Transfer) {
    trace!("http_pt_to_http");
    let mut server = match Server::new(&tf.server_addr).await {
        Ok(server) => server,
        Err(e) => {
            error!("Server error:{:?}", e);
            return;
        }
    };

    let (tx, rx) = oneshot::channel();
    state::hold(server.addr.to_string(), tx).await;

    let ra = RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion));
    let pd = Procedure::new(RouteFinder::new(ra, Protoc::TLS), Empty, Empty);
    server.tcp(pd, rx).await;
}

async fn http_pt_to_http_pt(tf: Transfer) {
    trace!("http_pt_to_http_pt");
    let mut server = match Server::new(&tf.server_addr).await {
        Ok(server) => server,
        Err(e) => {
            error!("Server error:{:?}", e);
            return;
        }
    };

    let (tx, rx) = oneshot::channel();
    state::hold(server.addr.to_string(), tx).await;

    let ra = RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion));
    let pd = Procedure::new(RouteFinder::new(ra, Protoc::TCP), Empty, Empty);
    server.tcp(pd, rx).await;
}

pub(crate) async fn http(tf: Transfer) {
    debug!("Server http start up");
    if Protoc::HTTP == tf.remote_protoc {
        http_to_http(tf).await;
    } else if Protoc::HTTPPT == tf.remote_protoc {
        http_to_http_pt(tf).await;
    }
}

pub(crate) async fn http_pt(tf: Transfer) {
    debug!("Server http_pt start up");
    if tf.remote_protoc == Protoc::HTTP {
        http_pt_to_http(tf).await;
    } else if tf.remote_protoc == Protoc::HTTPPT {
        http_pt_to_http_pt(tf).await;
    }
}
