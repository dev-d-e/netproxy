use crate::args::*;
use crate::core::*;
use crate::route::{self, Transfer};
use crate::state;
use crate::visit::{self, Visit};
use async_trait::async_trait;
use log::{debug, error, trace};
use std::fmt::Debug;
use std::net::SocketAddr;
use tokio::sync::oneshot::Sender;
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

static CURRENT_TX: OnceCell<Mutex<Vec<Sender<u8>>>> = OnceCell::const_new();

async fn current_tx() -> &'static Mutex<Vec<Sender<u8>>> {
    CURRENT_TX
        .get_or_init(|| async { Mutex::new(Vec::new()) })
        .await
}

static CURRENT_ADDR: OnceCell<Mutex<String>> = OnceCell::const_new();

async fn current_addr() -> &'static Mutex<String> {
    CURRENT_ADDR
        .get_or_init(|| async { Mutex::new(String::new()) })
        .await
}

static IP_SCOPE: OnceCell<Mutex<Vec<String>>> = OnceCell::const_new();

async fn ip_scope() -> &'static Mutex<Vec<String>> {
    IP_SCOPE
        .get_or_init(|| async { Mutex::new(Vec::new()) })
        .await
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
            trace!("CURRENT_TX send");
            //send stop signal to current server
            if let Some(tx) = current_tx().await.lock().await.pop() {
                if let Err(e) = tx.send(0) {
                    error!("CURRENT_TX send error:{:?}", e);
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
        RuleType::Transfer(o) => {
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
            if let Some(o) = o {
                return state::state_string(&o).await;
            } else {
                return state::list().await;
            }
        }
        RuleType::Shutdown(o) => {
            check_safe!();
            if !state::shutdown(&o).await {
                return OUTCOMES[7].to_string();
            }
            return OUTCOMES[0].to_string();
        }
        RuleType::None(n) => {
            check_safe!();
            return OUTCOMES[n as usize].to_string();
        }
    }
}

enum RuleType {
    Transfer(Transfer),
    Visit(Visit),
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
                                    let (ra, mut proportion) =
                                        some_addr_proportion(c, iter.next()).unwrap();
                                    divide(&mut proportion);
                                    let o = Transfer {
                                        server_protoc: p1,
                                        server_addr: b.to_string(),
                                        remote_protoc: p2,
                                        remote_addrs: ra,
                                        proportion,
                                    };
                                    return Self::Transfer(o);
                                } else {
                                    trace!("visit configuration");
                                    let o = Visit {
                                        server_protoc: p1,
                                        server_addr: b.to_string(),
                                        remote_protoc: p2,
                                    };
                                    return Self::Visit(o);
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
fn some_addr_proportion(a: &str, p: Option<&str>) -> Result<(Vec<String>, Vec<usize>), u8> {
    let addr: Vec<String> = a.split(',').map(|s| s.to_string()).collect();
    if addr
        .iter()
        .find(|s| s.parse::<SocketAddr>().is_err())
        .is_some()
    {
        return Err(3);
    }
    let proportion: Vec<usize> = if let Some(p) = p {
        let mut p: Vec<usize> = p.split(':').map(|s| s.parse().unwrap_or(0)).collect();
        if p.len() < addr.len() {
            p.extend_from_slice(&[addr.len() - p.len(); 1]);
        } else if p.len() > addr.len() {
            p.truncate(addr.len());
        }
        p
    } else {
        [addr.len(); 1].to_vec()
    };
    Ok((addr, proportion))
}

//if specify use tls connection, the first configuration sentence must be certificate sentence, then server will restart.
pub(crate) fn build(args: Args) {
    if args.is_safe() {
        unsafe {
            SAFE = true;
        }
    }

    let str = args.socket();
    let is_tool = args.is_tool();
    let ipscope = args.ipscope();

    tokiort_block_on(async {
        let mut server = match Server::new(str).await {
            Ok(server) => server,
            Err(e) => {
                error!("Server error:{:?}", e);
                return;
            }
        };

        if is_tool {
            spawn_tool(server.addr.to_string(), false);
        }

        current_addr()
            .await
            .lock()
            .await
            .push_str(&server.addr.to_string());
        ip_scope().await.lock().await.extend_from_slice(&ipscope);

        let pd = ProcedureService::new(Service::new(server.addr));

        let (tx, mut sc) = state::no_hold().await;
        current_tx().await.lock().await.push(tx);
        sc.get_scope().extend_from_slice(&ipscope);

        server.tcp(pd, sc).await;

        close_tool();
    });

    debug!("current server restart");
    tokiort_block_on(async {
        let mut server = match Server::new(str).await {
            Ok(server) => server,
            Err(e) => {
                error!("Server error:{:?}", e);
                return;
            }
        };

        if is_tool {
            spawn_tool(server.addr.to_string(), true);
        }

        let pd = ProcedureService::new(Service::new(server.addr));

        let (tx, mut sc) = state::no_hold().await;
        current_tx().await.lock().await.push(tx);
        sc.get_scope().extend_from_slice(&ipscope);

        let t = match valid_identity().await {
            Some(t) => t,
            None => {
                error!("server fail");
                return;
            }
        };
        server.tls(pd, t, sc).await;
    });
}

#[derive(Clone, Debug)]
struct Service {
    host: SocketAddr,
}

#[async_trait]
impl FuncRw for Service {
    async fn service(&mut self, req: &mut Vec<u8>, rsp: &mut Vec<u8>) {
        let str = into_str(req);
        req.clear();
        trace!("[{:?}]cfg:{:?}", &self.host, str);
        rsp.extend_from_slice(handle_cfg(str).await.as_bytes());
    }
}

impl Service {
    fn new(host: SocketAddr) -> Self {
        Self { host }
    }
}
