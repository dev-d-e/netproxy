use super::{
    get_index, into_str, FuncR, FuncRouteAlg, Procedure, Protoc, RouteFinder, Server, Transfer,
};
use crate::core::{find_header, parse_header};
use crate::{route_alg, transfer_data};
use async_trait::async_trait;
use log::{debug, error, info, trace};

route_alg!(
    RouteAlg,
    self,
    buf,
    {
        let str = into_str(buf);
        trace!("http request:{:?}", str);
        let headers = parse_header(&str);
        let host = find_header("Host", &headers);
        if host.is_none() {
            return None;
        }
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
    let server = Server::new(&tf.server_addr).await.unwrap();
    let pd = Procedure::new(
        RouteFinder::new(
            RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion)),
            Protoc::TLS,
        ),
        Empty,
    );
    info!("{:?} http_to_http", std::thread::current().id());
    server.tls(pd).await;
}

async fn http_to_http_pt(tf: Transfer) {
    let server = Server::new(&tf.server_addr).await.unwrap();
    let pd = Procedure::new(
        RouteFinder::new(
            RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion)),
            Protoc::TCP,
        ),
        Empty,
    );
    info!("{:?} http_to_http_pt", std::thread::current().id());
    server.tls(pd).await;
}

async fn http_pt_to_http(tf: Transfer) {
    let server = Server::new(&tf.server_addr).await.unwrap();
    let pd = Procedure::new(
        RouteFinder::new(
            RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion)),
            Protoc::TLS,
        ),
        Empty,
    );
    info!("{:?} http_pt_to_http", std::thread::current().id());
    server.tcp(pd).await;
}

async fn http_pt_to_http_pt(tf: Transfer) {
    let server = Server::new(&tf.server_addr).await.unwrap();
    let pd = Procedure::new(
        RouteFinder::new(
            RouteAlg(tf.remote_addrs, 0, get_index(tf.proportion)),
            Protoc::TCP,
        ),
        Empty,
    );
    info!("{:?} http_pt_to_http_pt", std::thread::current().id());
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
