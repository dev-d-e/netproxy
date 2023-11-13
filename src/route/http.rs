use super::{
    get_identity, get_index, into_str, FuncR, FuncRouteAlg, Procedure, Protoc, RouteFinder, Server,
    Transfer,
};
use crate::{route_alg, transfer_data};
use async_trait::async_trait;
use log::{debug, error, trace};

route_alg!(
    RouteAlg,
    self,
    buf,
    {
        let str = into_str(buf);
        trace!("http request:{:?}", str);

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
    let server = match Server::new(&tf.server_addr).await {
        Ok(server) => server,
        Err(e) => {
            error!("Server error:{:?}", e);
            return;
        }
    };
    let pd = Procedure::new(
        RouteFinder::new(
            RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion)),
            Protoc::TLS,
        ),
        Empty,
        Empty,
    );
    let t = match get_identity() {
        Some(t) => t,
        None => {
            error!("server fail");
            return;
        }
    };
    server.tls(pd, t).await;
}

async fn http_to_http_pt(tf: Transfer) {
    trace!("http_to_http_pt");
    let server = match Server::new(&tf.server_addr).await {
        Ok(server) => server,
        Err(e) => {
            error!("Server error:{:?}", e);
            return;
        }
    };
    let pd = Procedure::new(
        RouteFinder::new(
            RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion)),
            Protoc::TCP,
        ),
        Empty,
        Empty,
    );
    let t = match get_identity() {
        Some(t) => t,
        None => {
            error!("server fail");
            return;
        }
    };
    server.tls(pd, t).await;
}

async fn http_pt_to_http(tf: Transfer) {
    trace!("http_pt_to_http");
    let server = match Server::new(&tf.server_addr).await {
        Ok(server) => server,
        Err(e) => {
            error!("Server error:{:?}", e);
            return;
        }
    };
    let pd = Procedure::new(
        RouteFinder::new(
            RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion)),
            Protoc::TLS,
        ),
        Empty,
        Empty,
    );
    server.tcp(pd).await;
}

async fn http_pt_to_http_pt(tf: Transfer) {
    trace!("http_pt_to_http_pt");
    let server = match Server::new(&tf.server_addr).await {
        Ok(server) => server,
        Err(e) => {
            error!("Server error:{:?}", e);
            return;
        }
    };
    let pd = Procedure::new(
        RouteFinder::new(
            RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion)),
            Protoc::TCP,
        ),
        Empty,
        Empty,
    );
    server.tcp(pd).await;
}

pub(crate) async fn accept(tf: Transfer) {
    if Protoc::HTTP == tf.server_protoc {
        debug!("Server http start up");
        if Protoc::HTTP == tf.remote_protoc {
            http_to_http(tf).await;
        } else if Protoc::HTTPPT == tf.remote_protoc {
            http_to_http_pt(tf).await;
        }
    } else if Protoc::HTTPPT == tf.server_protoc {
        debug!("Server http_pt start up");
        if Protoc::HTTP == tf.remote_protoc {
            http_pt_to_http(tf).await;
        } else if Protoc::HTTPPT == tf.remote_protoc {
            http_pt_to_http_pt(tf).await;
        }
    }
}
