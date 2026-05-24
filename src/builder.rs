use crate::args::*;
use crate::core::*;
use crate::route::{self, *};
use crate::state::{self, *};
use crate::visit::{self, *};
use async_trait::async_trait;
use std::fmt::Debug;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::sync::{OnceCell, RwLock};

const HTTP: &str = "http";
const HTTP_PT: &str = "http_pt";
const TCP: &str = "tcp";
const TLS: &str = "tls";
const UDP: &str = "udp";
const CERTIFICATE: &str = "certificate";
const STATE: &str = "state";
const SHUTDOWN: &str = "shutdown";

const OUTCOMES: [&[u8]; 8] = [
    b"ok",
    b"configuration sentences error",
    b"protocol error",
    b"SocketAddr error",
    b"must be certificate sentence",
    b"certificate error",
    b"server is starting up...",
    b"shutdown error",
];

static mut SAFE: bool = false;

static CURRENT: OnceCell<RwLock<Option<ServerState>>> = OnceCell::const_new();

async fn current() -> &'static RwLock<Option<ServerState>> {
    CURRENT.get_or_init(|| async { RwLock::new(None) }).await
}

fn to_protoc(s: &str) -> Option<Protoc> {
    match s {
        HTTP => Some(Protoc::HTTP),
        HTTP_PT => Some(Protoc::HTTPPT),
        TCP => Some(Protoc::TCP),
        TLS => Some(Protoc::TLS),
        UDP => Some(Protoc::UDP),
        _ => None,
    }
}

async fn send_stop() {
    unsafe {
        if SAFE {
            trace!("Close send");
            //send stop signal to current server
            if let Some(c) = current().await.write().await.take() {
                if let Err(_) = c.send(ControlInfo::Close) {
                    error!("Close send error");
                }
            }

            SAFE = false;
        }
    }
}

enum RuleType {
    Route(RouteInfo),
    Visit(VisitInfo),
    CertificateF(String, String),
    CertificateS(String, String),
    State(Option<SocketAddr>),
    Shutdown(SocketAddr),
}

impl TryFrom<&str> for RuleType {
    type Error = usize;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        trace!("configuration check");
        let mut iter = s.split_whitespace();
        if let Some(a) = iter.next() {
            match a {
                CERTIFICATE => {
                    if let Some(b) = iter.next() {
                        if let Some(c) = iter.next() {
                            if let Some(d) = iter.next() {
                                match b {
                                    "f" => {
                                        trace!("certificate configuration");
                                        return Ok(Self::CertificateF(
                                            c.to_string(),
                                            d.to_string(),
                                        ));
                                    }
                                    "s" => {
                                        trace!("certificate configuration");
                                        return Ok(Self::CertificateS(
                                            c.to_string(),
                                            d.to_string(),
                                        ));
                                    }
                                    _ => {
                                        return Err(5);
                                    }
                                }
                            }
                        }
                    }
                }
                STATE => {
                    trace!("state configuration");
                    return Ok(Self::State(
                        iter.next().and_then(|s| s.parse::<SocketAddr>().ok()),
                    ));
                }
                SHUTDOWN => {
                    if let Some(b) = iter.next().and_then(|b| b.parse::<SocketAddr>().ok()) {
                        trace!("shutdown configuration");
                        return Ok(Self::Shutdown(b));
                    }
                }
                _ => {
                    //accept protocol and route target protocol, split by '-', if only one,the other is the same.
                    let (p1, p2) = a.split_once('-').unwrap_or((a, a));
                    if let Some(p1) = to_protoc(p1) {
                        if let Some(p2) = to_protoc(p2) {
                            if let Some(b) = iter.next() {
                                if b.parse::<SocketAddr>().is_err() {
                                    return Err(3);
                                }
                                if let Some(c) = iter.next() {
                                    trace!("transfer configuration");
                                    if c.is_empty() {
                                        return Err(3);
                                    }
                                    let (ra, mut proportion) = some_addr_proportion(c, iter.next());
                                    divide(&mut proportion);
                                    return Ok(Self::Route(RouteInfo::new(
                                        p1,
                                        b.to_string(),
                                        p2,
                                        ra,
                                        proportion,
                                    )));
                                } else {
                                    trace!("visit configuration");
                                    return Ok(Self::Visit(VisitInfo::new(p1, b.to_string(), p2)));
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(1)
    }
}

macro_rules! check_safe {
    ($a:ident) => {
        unsafe {
            if SAFE {
                $a.extend_from_slice(OUTCOMES[4]);
                return;
            }
        }
    };
}

impl RuleType {
    async fn handle(self, rsp: &mut Vec<u8>) {
        match self {
            Self::Route(o) => {
                check_safe!(rsp);
                route::start_up(o);
                rsp.extend_from_slice(OUTCOMES[6]);
            }
            Self::Visit(o) => {
                check_safe!(rsp);
                visit::start_up(o);
                rsp.extend_from_slice(OUTCOMES[6]);
            }
            Self::CertificateF(a, b) => {
                if build_certificate_from_file(a, b).await {
                    error!("certificate error");
                    rsp.extend_from_slice(OUTCOMES[5]);
                    return;
                }
                send_stop().await;
                rsp.extend_from_slice(OUTCOMES[0]);
            }
            Self::CertificateS(a, b) => {
                if build_certificate_from_socket(a, b).await {
                    error!("certificate error");
                    rsp.extend_from_slice(OUTCOMES[5]);
                    return;
                }
                send_stop().await;
                rsp.extend_from_slice(OUTCOMES[0]);
            }
            Self::State(o) => {
                check_safe!(rsp);
                if let Some(o) = &o {
                    rsp.extend_from_slice(state::state_string(o).await.as_bytes());
                } else {
                    rsp.extend_from_slice(state::list().await.as_bytes());
                }
            }
            Self::Shutdown(o) => {
                check_safe!(rsp);
                if state::shutdown(&o).await {
                    rsp.extend_from_slice(OUTCOMES[0]);
                } else {
                    rsp.extend_from_slice(OUTCOMES[7]);
                }
            }
        }
    }
}

//target socket addr, one or several, where data transfer to, split by ','
//proportion of data transfer to target, split by ':'. it's digit and correspondence with third. if it's not digit, replace with 0. if number is less than socket addrs, fill with 1. it can be omitted.
fn some_addr_proportion(a: &str, p: Option<&str>) -> (Vec<String>, Vec<usize>) {
    let addr: Vec<String> = a.split(',').map(|s| s.to_string()).collect();
    let addr_len = addr.len();

    let proportion: Vec<usize> = if let Some(p) = p {
        let mut p: Vec<usize> = p.split(':').map(|s| s.parse().unwrap_or(0)).collect();
        if p.len() < addr_len {
            p.extend_from_slice(&[addr_len - p.len(); 1]);
        } else if p.len() > addr_len {
            p.truncate(addr_len);
        }
        p
    } else {
        [addr_len; 1].to_vec()
    };
    (addr, proportion)
}

//if specify use tls connection, the first configuration sentence must be certificate sentence, then server will restart.
pub(crate) fn build(args: Args) {
    if args.is_safe() {
        unsafe {
            SAFE = true;
        }
    }

    let addr = args.socket();
    let is_tool = args.is_tool();
    let ipscope = args.ipscope();

    tokiort_block_on(async { start_up(addr, is_tool, &ipscope, false).await });

    if args.is_safe() {
        debug!("current server restart");
        tokiort_block_on(async { start_up(addr, is_tool, &ipscope, true).await });
    }
}

async fn start_up(addr: &str, is_tool: bool, ipscope: &Vec<IpAddr>, is_safe: bool) {
    let (mut server, a, mut b) = if let Some(server) = Server::new(addr).await {
        server
    } else {
        return;
    };
    server.set_ip_scope(ipscope);

    if is_tool {
        spawn_tool(server.addr(), false);
    }

    current()
        .await
        .write()
        .await
        .replace(ServerState::new(*server.addr(), a));

    tokio::spawn(async move {
        while let Some(o) = b.recv().await {
            match o {
                StateInfo::Sum(velocity, date_time) => {
                    if let Some(ss) = current().await.write().await.as_mut() {
                        ss.set_velocity(velocity);
                        *ss.date_time_mut() = date_time;
                    }
                }
            }
        }
        trace!("current state end");
    });

    if is_safe {
        if let Some(t) = get_tls_acceptor().await {
            let o = ServiceTls::new(MainService::new(*server.addr()), t);
            server.accept(o).await;
        } else {
            error!("error end");
        }
    } else {
        let o = Service::new(MainService::new(*server.addr()));
        server.accept(o).await;
    }

    if is_tool {
        close_tool();
    }
}

#[derive(Clone, Debug)]
struct MainService {
    host: Arc<SocketAddr>,
}

#[async_trait]
impl FuncRw for MainService {
    async fn service(&mut self, req: &mut Vec<u8>, rsp: &mut Vec<u8>) {
        let s = into_str(req);
        req.clear();
        trace!("[{:?}]cfg:{:?}", &self.host, s);
        match RuleType::try_from(s.as_str()) {
            Ok(o) => {
                o.handle(rsp).await;
            }
            Err(n) => {
                check_safe!(rsp);
                rsp.extend_from_slice(OUTCOMES[n]);
            }
        }
    }
}

impl MainService {
    fn new(host: SocketAddr) -> Self {
        Self {
            host: Arc::new(host),
        }
    }
}
