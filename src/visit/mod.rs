use crate::core::{
    self, get_identity, into_str, parse_request, FuncR, FuncRemote, Procedure, Protoc, Server,
};
use crate::transfer_data;
use async_trait::async_trait;
use log::{debug, error, trace};

#[derive(Clone, Debug)]
struct VisitFinder(Protoc);

#[async_trait]
impl FuncRemote for VisitFinder {
    async fn get(&self, buf: &mut Vec<u8>) -> Option<(Protoc, String, usize)> {
        let str = into_str(buf);
        trace!("http request:{:?}", str);
        let req = match parse_request(&str.as_bytes()) {
            Ok(req) => req,
            Err(e) => {
                error!("http request error:{:?}", e);
                return None;
            }
        };
        let host = req.find_host_value()?;
        Some((self.0.clone(), host + ":443", 0))
    }
}

transfer_data!(Empty, self, _buf, {}, {});

#[derive(Debug)]
pub(crate) struct Visit {
    pub(crate) server_protoc: Protoc,
    pub(crate) server_addr: String,
}

pub(crate) fn start_up(vt: Visit) {
    debug!("{:?}", vt);
    core::new_thread_tokiort_block_on(async move {
        match vt.server_protoc {
            Protoc::HTTP => tls(vt).await,
            Protoc::HTTPPT => tcp(vt).await,
            _ => {}
        }
    });
}

async fn tls(vt: Visit) {
    debug!("Server tls start up");
    let server = match Server::new(&vt.server_addr).await {
        Ok(server) => server,
        Err(e) => {
            error!("Server error:{:?}", e);
            return;
        }
    };
    let pd = Procedure::new(VisitFinder(Protoc::TLS), Empty, Empty);
    let t = match get_identity() {
        Some(t) => t,
        None => {
            error!("Server fail");
            return;
        }
    };
    server.tls(pd, t).await;
}

async fn tcp(vt: Visit) {
    debug!("Server tcp start up");
    let server = match Server::new(&vt.server_addr).await {
        Ok(server) => server,
        Err(e) => {
            error!("Server error:{:?}", e);
            return;
        }
    };
    let pd = Procedure::new(VisitFinder(Protoc::TLS), Empty, Empty);
    server.tcp(pd).await;
}
