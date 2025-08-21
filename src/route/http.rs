use super::*;
use log::debug;

struct RouteAlg(Vec<String>, usize, Vec<usize>);

impl FuncRouteAlg for RouteAlg {
    fn addr(&mut self) -> (String, String) {
        let s = self.0[self.2[self.1]].clone();
        self.1 = (self.1 + 1) % self.2.len();
        let h = s.split(':').next().unwrap_or_default().to_string();
        (s, h)
    }
}

pub(super) async fn http(r: RouteInfo) {
    debug!("server http start up");

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

pub(super) async fn http_pt(r: RouteInfo) {
    debug!("server http_pt start up");

    let ra = RouteAlg(r.remote_addrs, 0, get_index(r.proportion));
    let o = Route::new(
        RouteFinder::new(ra, r.remote_protoc),
        RouteR::new(),
        RouteR::new(),
    );
    server_accept(&r.server_addr, o).await;
}
