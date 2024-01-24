use crate::args::*;
use crate::core::*;
use crate::route::{self, Transfer};
use crate::rw_service;
use crate::state;
use crate::visit::{self, Visit};
use async_trait::async_trait;
use log::{debug, error, trace};
use regex::Regex;
use std::collections::HashMap;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::OnceLock;
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

const KEYWORDS: [(&str, CfgType); 3] = [
    (CERTIFICATE, CfgType::Certificate),
    (STATE, CfgType::State),
    (SHUTDOWN, CfgType::Shutdown),
];

const PROTOCS: [(&str, Protoc); 5] = [
    (HTTP, Protoc::HTTP),
    (HTTP_PT, Protoc::HTTPPT),
    (TCP, Protoc::TCP),
    (TLS, Protoc::TLS),
    (UDP, Protoc::UDP),
];

const OUTCOMES: [(u8, &str); 8] = [
    (0, "ok"),
    (1, "configuration sentences error"),
    (2, "protocol error"),
    (3, "SocketAddr error"),
    (4, "must be certificate sentence"),
    (5, "certificate error"),
    (6, "server is starting up..."),
    (7, "shutdown error"),
];

static mut SAFE: bool = false;

static REGEX_PROTOCOL: OnceCell<Regex> = OnceCell::const_new();

async fn regex_protocol() -> &'static Regex {
    REGEX_PROTOCOL
        .get_or_init(|| async {
            Regex::new(r"^(http|http_pt|tcp|tls|udp)(-(http|http_pt|tcp|tls|udp)|-|)$").unwrap()
        })
        .await
}

static REGEX_VISIT_PROTOCOL: OnceLock<Regex> = OnceLock::new();

fn regex_visit_protocol() -> &'static Regex {
    REGEX_VISIT_PROTOCOL
        .get_or_init(|| Regex::new(r"^(http|http_pt)(-(http|http_pt)|-|)$").unwrap())
}

static REGEX_CERTIFICATE: OnceLock<Regex> = OnceLock::new();

fn regex_certificate() -> &'static Regex {
    REGEX_CERTIFICATE.get_or_init(|| Regex::new(r"^certificate (f|s) ").unwrap())
}

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

static KW_MAP: OnceCell<Mutex<HashMap<&'static str, CfgType>>> = OnceCell::const_new();

async fn kw_map() -> &'static Mutex<HashMap<&'static str, CfgType>> {
    KW_MAP
        .get_or_init(|| async { Mutex::new(HashMap::from(KEYWORDS)) })
        .await
}

static P_MAP: OnceCell<Mutex<HashMap<&'static str, Protoc>>> = OnceCell::const_new();

async fn p_map() -> &'static Mutex<HashMap<&'static str, Protoc>> {
    P_MAP
        .get_or_init(|| async { Mutex::new(HashMap::from(PROTOCS)) })
        .await
}

static OC_MAP: OnceCell<Mutex<HashMap<u8, &'static str>>> = OnceCell::const_new();

async fn oc_map() -> &'static Mutex<HashMap<u8, &'static str>> {
    OC_MAP
        .get_or_init(|| async { Mutex::new(HashMap::from(OUTCOMES)) })
        .await
}

static CHECKERS: OnceCell<
    Mutex<HashMap<CfgType, Box<dyn (Fn(&str, &Vec<String>) -> bool) + Send + Sync + 'static>>>,
> = OnceCell::const_new();

async fn checkers() -> &'static Mutex<
    HashMap<CfgType, Box<dyn (Fn(&str, &Vec<String>) -> bool) + Send + Sync + 'static>>,
> {
    CHECKERS
        .get_or_init(|| async {
            let mut map: HashMap<
                CfgType,
                Box<dyn (Fn(&str, &Vec<String>) -> bool) + Send + Sync + 'static>,
            > = HashMap::new();
            map.insert(
                CfgType::Transfer,
                Box::new(|_s, v| {
                    if v.len() > 2 {
                        if let Ok(_) = v[1].parse::<SocketAddr>() {
                            trace!("transfer configuration");
                            return true;
                        }
                    }
                    false
                }),
            );
            map.insert(
                CfgType::Visit,
                Box::new(|_s, v| {
                    if v.len() == 2 && regex_visit_protocol().is_match(&v[0]) {
                        if let Ok(_) = v[1].parse::<SocketAddr>() {
                            trace!("visit configuration");
                            return true;
                        }
                    }
                    false
                }),
            );
            map.insert(
                CfgType::Certificate,
                Box::new(|s, v| {
                    if v.len() == 4 && regex_certificate().is_match(s) {
                        trace!("certificate configuration");
                        return true;
                    }
                    false
                }),
            );
            map.insert(
                CfgType::State,
                Box::new(|_s, v| {
                    let n = v.len();
                    if n == 1 || n == 2 {
                        trace!("state configuration");
                        return true;
                    }
                    false
                }),
            );
            map.insert(
                CfgType::Shutdown,
                Box::new(|_s, v| {
                    if v.len() == 2 {
                        trace!("shutdown configuration");
                        return true;
                    }
                    false
                }),
            );
            Mutex::new(map)
        })
        .await
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum CfgType {
    Transfer,
    Visit,
    Certificate,
    State,
    Shutdown,
}

async fn get_outcome(k: u8) -> String {
    let map = oc_map().await.lock().await;
    match map.get(&k) {
        Some(s) => s.to_string(),
        None => "nothing".to_string(),
    }
}

async fn handle_cfg(str: String) -> String {
    match check(&str).await {
        Ok((ct, v)) => {
            trace!("handle configuration: {:?} {:?}", ct, v);
            unsafe {
                if SAFE && ct != CfgType::Certificate {
                    return get_outcome(4).await;
                }
            }
            match take_effect(ct, v).await {
                Ok(s) => {
                    let mut ss = String::from("ok ");
                    ss.push_str(&s);
                    return ss;
                }
                Err(e) => return get_outcome(e).await,
            }
        }
        Err(e) => return get_outcome(e).await,
    };
}

async fn check(str: &str) -> Result<(CfgType, Vec<String>), u8> {
    let v: Vec<String> = str.split_whitespace().map(|s| s.to_string()).collect();
    let n = v.len();
    trace!("configuration check");
    if n > 0 {
        let ct = if regex_protocol().await.is_match(&v[0]) {
            if n > 2 {
                CfgType::Transfer
            } else if n == 2 {
                CfgType::Visit
            } else {
                return Err(1);
            }
        } else {
            let map = kw_map().await.lock().await;
            match map.get(v[0].as_str()) {
                Some(ct) => ct.clone(),
                None => return Err(1),
            }
        };
        let map = checkers().await.lock().await;
        if let Some(checker) = map.get(&ct) {
            if checker(str, &v) {
                trace!("pass");
                return Ok((ct, v));
            }
        }
    }
    Err(1)
}

async fn take_effect(ct: CfgType, v: Vec<String>) -> Result<String, u8> {
    match ct {
        CfgType::Transfer => {
            let tf = to_transfer(v).await?;
            route::start_up(tf);
            return Err(6);
        }
        CfgType::Visit => {
            let vi = to_visit(v).await?;
            visit::start_up(vi);
            return Err(6);
        }
        CfgType::Certificate => {
            match v[1].as_str() {
                "f" => {
                    if let Err(e) = build_certificate_from_file(v[2].clone(), v[3].clone()).await {
                        error!("certificate error:{:?}", e);
                        return Err(5);
                    }
                }
                "s" => {
                    if let Err(e) = build_certificate_from_socket(v[2].clone(), v[3].clone()).await
                    {
                        error!("certificate error:{:?}", e);
                        return Err(5);
                    }
                }
                _ => {
                    return Err(5);
                }
            }
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
        CfgType::State => {
            if v.len() == 1 {
                let s = state::list().await;
                return Ok(s);
            } else if v.len() == 2 {
                let s = state::state_string(&v[1]).await;
                return Ok(s);
            }
        }
        CfgType::Shutdown => {
            if !state::shutdown(&v[1]).await {
                return Err(7);
            }
        }
    }
    Err(0)
}

async fn to_transfer(v: Vec<String>) -> Result<Transfer, u8> {
    let (p1, p2) = two_protoc(&v[0]).await?;

    let (ra, proportion) = some_addr_proportion(&v)?;

    Ok(Transfer {
        server_protoc: p1,
        server_addr: v[1].clone(),
        remote_protoc: p2,
        remote_addrs: ra,
        proportion,
    })
}

async fn to_visit(v: Vec<String>) -> Result<Visit, u8> {
    let (p1, p2) = two_protoc(&v[0]).await?;

    Ok(Visit {
        server_protoc: p1,
        server_addr: v[1].clone(),
        remote_protoc: p2,
    })
}

//accept protocol and route target protocol, split by '-', if only one,the other is the same.
async fn two_protoc(str: &str) -> Result<(Protoc, Protoc), u8> {
    let pv: Vec<&str> = str.split('-').collect();
    let p1 = to_protoc(pv[0]).await?;
    let p2 = if pv.len() > 1 {
        to_protoc(pv[1]).await?
    } else {
        p1.clone()
    };
    Ok((p1, p2))
}

async fn to_protoc(str: &str) -> Result<Protoc, u8> {
    let map = p_map().await.lock().await;
    match map.get(&str) {
        Some(p) => Ok(p.clone()),
        None => Err(2),
    }
}

//target socket addr, one or several, where data transfer to, split by ','
//proportion of data transfer to target, split by ':'. it's digit and correspondence with third. if it's not digit, replace with 0. if number is less than socket addrs, fill with 1. it can be omitted.
fn some_addr_proportion(v: &Vec<String>) -> Result<(Vec<String>, Vec<usize>), u8> {
    let addr: Vec<String> = v[2].split(',').map(|s| s.to_string()).collect();
    if addr
        .iter()
        .find(|s| s.parse::<SocketAddr>().is_err())
        .is_some()
    {
        return Err(3);
    }
    let proportion: Vec<usize> = if v.len() > 3 {
        let mut p: Vec<usize> = v[3].split(':').map(|s| s.parse().unwrap_or(0)).collect();
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

        let pd = ProcedureService::new(Service);

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

        let pd = ProcedureService::new(Service);

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

rw_service!(Service, self, req, rsp, {
    let str = into_str(req);
    req.clear();
    rsp.extend_from_slice(handle_cfg(str).await.as_bytes());
});
