use crate::core::*;
use crate::state;
use crate::transfer_data;
use async_trait::async_trait;
use log::{debug, error};

#[derive(Clone, Debug)]
struct VisitFinder(Protoc);

#[async_trait]
impl FuncRemote for VisitFinder {
    async fn get(&self, buf: &mut Vec<u8>) -> Option<(Protoc, String, u32)> {
        let req = match parse_request(buf) {
            Ok(req) => req,
            Err(e) => {
                error!("http request error:{:?}", e);
                return None;
            }
        };
        let mut host = req.find_host_value()?;
        if self.0 == Protoc::HTTP {
            host = http_port(host);
            Some((Protoc::TLS, host, 0))
        } else if self.0 == Protoc::HTTPPT {
            host = http_pt_port(host);
            Some((Protoc::TCP, host, 0))
        } else {
            None
        }
    }
}

fn http_pt_port(mut str: String) -> String {
    if let None = str.find(':') {
        str.push_str(":80");
    }
    str
}

fn http_port(mut str: String) -> String {
    if let None = str.find(':') {
        str.push_str(":443");
    }
    str
}

transfer_data!(Empty, self, _buf, {}, {});

#[derive(Debug)]
pub(crate) struct Visit {
    pub(crate) server_protoc: Protoc,
    pub(crate) server_addr: String,
    pub(crate) remote_protoc: Protoc,
}

pub(crate) fn start_up(vt: Visit) {
    debug!("{:?}", vt);
    new_thread_tokiort_block_on(async move {
        match vt.server_protoc {
            Protoc::HTTP => http(vt).await,
            Protoc::HTTPPT => http_pt(vt).await,
            _ => {}
        }
    });
}

async fn http(vt: Visit) {
    debug!("Server tls start up");
    let i = match valid_identity().await {
        Some(i) => i,
        None => {
            error!("Server fail");
            return;
        }
    };

    let mut server = match Server::new(&vt.server_addr).await {
        Ok(server) => server,
        Err(e) => {
            error!("Server error:{:?}", e);
            return;
        }
    };

    let sc = state::hold(server.addr.to_string()).await;

    let pd = Procedure::new(VisitFinder(vt.remote_protoc), Empty, Empty);
    server.tls(pd, i, sc).await;
}

async fn http_pt(vt: Visit) {
    debug!("Server tcp start up");
    let mut server = match Server::new(&vt.server_addr).await {
        Ok(server) => server,
        Err(e) => {
            error!("Server error:{:?}", e);
            return;
        }
    };

    let sc = state::hold(server.addr.to_string()).await;

    let pd = Procedure::new(VisitFinder(vt.remote_protoc), Empty, Empty);
    server.tcp(pd, sc).await;
}
