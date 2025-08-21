use crate::args::*;
use crate::core::*;
use crate::route::{self, *};
use crate::state::{self, *};
use crate::visit::{self, *};
use async_trait::async_trait;
use log::{debug, error, trace};
use std::fmt::Debug;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};

const HTTP: &str = "http";
const HTTP_PT: &str = "http_pt";
const TCP: &str = "tcp";
const TLS: &str = "tls";
const UDP: &str = "udp";
const CERTIFICATE: &str = "certificate";
const STATE: &str = "state";
const SHUTDOWN: &str = "shutdown";

const OUTCOMES: [&str; 8] = [
    "ok",
    "configuration sentences error",
    "protocol error",
    "SocketAddr error",
    "must be certificate sentence",
    "certificate error",
    "server is starting up...",
    "shutdown error",
];

static mut SAFE: bool = false;

static CURRENT: OnceCell<Mutex<Option<ServerState>>> = OnceCell::const_new();

async fn current() -> &'static Mutex<Option<ServerState>> {
    CURRENT.get_or_init(|| async { Mutex::new(None) }).await
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
            if let Some(c) = current().await.lock().await.take() {
                if let Err(_) = c.send(ControlInfo::Close) {
                    error!("Close send error");
                }
            }

            SAFE = false;
        }
    }
}

macro_rules! check_safe {
    () => {
        unsafe {
            if SAFE {
                return OUTCOMES[4].to_string();
            }
        }
    };
}

async fn handle_cfg(str: String) -> String {
    match RuleType::from(str) {
        RuleType::Route(o) => {
            check_safe!();
            route::start_up(o);
            return OUTCOMES[6].to_string();
        }
        RuleType::Visit(o) => {
            check_safe!();
            visit::start_up(o);
            return OUTCOMES[6].to_string();
        }
        RuleType::CertificateF(a, b) => {
            if let Err(e) = build_certificate_from_file(a, b).await {
                error!("certificate error:{:?}", e);
                return OUTCOMES[5].to_string();
            }
            send_stop().await;
            return OUTCOMES[0].to_string();
        }
        RuleType::CertificateS(a, b) => {
            if let Err(e) = build_certificate_from_socket(a, b).await {
                error!("certificate error:{:?}", e);
                return OUTCOMES[5].to_string();
            }
            send_stop().await;
            return OUTCOMES[0].to_string();
        }
        RuleType::State(o) => {
            check_safe!();
            if let Some(o) = o.and_then(|o| o.parse::<SocketAddr>().ok()) {
                return state::state_string(&o).await;
            } else {
                return state::list().await;
            }
        }
        RuleType::Shutdown(o) => {
            check_safe!();
            if let Ok(o) = o.parse::<SocketAddr>() {
                if state::shutdown(&o).await {
                    return OUTCOMES[0].to_string();
                }
            }
            return OUTCOMES[7].to_string();
        }
        RuleType::None(n) => {
            check_safe!();
            return OUTCOMES[n as usize].to_string();
        }
    }
}

enum RuleType {
    Route(RouteInfo),
    Visit(VisitInfo),
    CertificateF(String, String),
    CertificateS(String, String),
    State(Option<String>),
    Shutdown(String),
    None(u8),
}

impl RuleType {
    pub(crate) fn from(s: String) -> Self {
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
                                        return Self::CertificateF(c.to_string(), d.to_string());
                                    }
                                    "s" => {
                                        trace!("certificate configuration");
                                        return Self::CertificateS(c.to_string(), d.to_string());
                                    }
                                    _ => {
                                        return Self::None(5);
                                    }
                                }
                            }
                        }
                    }
                }
                STATE => {
                    trace!("state configuration");
                    return Self::State(iter.next().map(|s| s.to_string()));
                }
                SHUTDOWN => {
                    if let Some(b) = iter.next() {
                        trace!("shutdown configuration");
                        return Self::Shutdown(b.to_string());
                    }
                }
                _ => {
                    //accept protocol and route target protocol, split by '-', if only one,the other is the same.
                    let (p1, p2) = a.split_once('-').unwrap_or((a, a));
                    if let Some(p1) = to_protoc(p1) {
                        if let Some(p2) = to_protoc(p2) {
                            if let Some(b) = iter.next() {
                                if b.parse::<SocketAddr>().is_err() {
                                    return Self::None(3);
                                }
                                if let Some(c) = iter.next() {
                                    trace!("transfer configuration");
                                    if c.is_empty() {
                                        return Self::None(3);
                                    }
                                    let (ra, mut proportion) = some_addr_proportion(c, iter.next());
                                    divide(&mut proportion);
                                    return Self::Route(RouteInfo::new(
                                        p1,
                                        b.to_string(),
                                        p2,
                                        ra,
                                        proportion,
                                    ));
                                } else {
                                    trace!("visit configuration");
                                    return Self::Visit(VisitInfo::new(p1, b.to_string(), p2));
                                }
                            }
                        }
                    }
                }
            }
        }
        Self::None(1)
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
    let (mut server, a, mut b) = if let Ok(server) = Server::new(addr)
        .await
        .inspect_err(|e| error!("new server: {:?}", e))
    {
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
        .lock()
        .await
        .replace(ServerState::new(*server.addr(), a));

    tokio::spawn(async move {
        while let Some(o) = b.recv().await {
            match o {
                StateInfo::Sum(velocity, date_time) => {
                    if let Some(ss) = current().await.lock().await.as_mut() {
                        ss.set_velocity(velocity);
                        *ss.date_time_mut() = date_time;
                    }
                }
            }
        }
        trace!("current state end");
    });

    if is_safe {
        if let Ok(t) = get_tls_acceptor().await {
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
        rsp.extend_from_slice(handle_cfg(s).await.as_bytes());
    }
}

impl MainService {
    fn new(host: SocketAddr) -> Self {
        Self {
            host: Arc::new(host),
        }
    }
}
