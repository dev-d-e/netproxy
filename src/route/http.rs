use super::*;
use log::{debug, error, trace};

struct RouteAlg(Vec<String>, usize, Vec<usize>);

impl FuncRouteAlg for RouteAlg {
    fn addr(&mut self, buf: &mut Vec<u8>) -> Option<String> {
        let _req = match HttpRequest::parse(buf) {
            Ok(req) => req,
            Err(e) => {
                error!("http request error:{:?}", e);
                return None;
            }
        };

        let s = self.0[self.2[self.1]].clone();
        self.1 = (self.1 + 1) % self.2.len();
        Some(s)
    }
}

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

    let sc = state::hold(server.addr.to_string()).await;

    let ra = RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion));
    let pd = Procedure::new(
        RouteFinder::new(ra, Protoc::TLS),
        RouteR::new(),
        RouteR::new(),
    );
    server.tls(pd, i, sc).await;
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

    let sc = state::hold(server.addr.to_string()).await;

    let ra = RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion));
    let pd = Procedure::new(
        RouteFinder::new(ra, Protoc::TCP),
        RouteR::new(),
        RouteR::new(),
    );
    server.tls(pd, i, sc).await;
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

    let sc = state::hold(server.addr.to_string()).await;

    let ra = RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion));
    let pd = Procedure::new(
        RouteFinder::new(ra, Protoc::TLS),
        RouteR::new(),
        RouteR::new(),
    );
    server.tcp(pd, sc).await;
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

    let sc = state::hold(server.addr.to_string()).await;

    let ra = RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion));
    let pd = Procedure::new(
        RouteFinder::new(ra, Protoc::TCP),
        RouteR::new(),
        RouteR::new(),
    );
    server.tcp(pd, sc).await;
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
