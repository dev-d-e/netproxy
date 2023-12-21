use crate::core::{now_str, FuncControl};
use async_trait::async_trait;
use lazy_static::lazy_static;
use log::error;
use std::collections::HashMap;
use std::net::IpAddr;

use tokio::sync::oneshot;
use tokio::sync::Mutex;

lazy_static! {
    static ref SERVER_STATE: Mutex<HashMap<String, ServerState>> = Mutex::new(HashMap::new());
}

#[derive(Debug)]
struct ServerState {
    server: String,
    tx: oneshot::Sender<u8>,
    velocity: u32,
    date_time: String,
}

impl ServerState {
    fn new(server: String, tx: oneshot::Sender<u8>) -> Self {
        ServerState {
            server,
            tx,
            velocity: 0,
            date_time: String::new(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ServerControl {
    server: String,
    rx: Vec<oneshot::Receiver<u8>>,
    ip_scope: Vec<String>,
}

#[async_trait]
impl FuncControl for ServerControl {
    fn stop_receiver(&mut self) -> oneshot::Receiver<u8> {
        if let Some(rx) = self.rx.pop() {
            rx
        } else {
            let (_, rx) = oneshot::channel();
            rx
        }
    }

    async fn reject_ip(&self, ip: IpAddr) -> bool {
        if self.ip_scope.is_empty() {
            return false;
        }
        for s in &self.ip_scope {
            if s == &ip.to_string() {
                return false;
            }
        }
        true
    }

    async fn velocity(&self, velocity: u32) {
        if let Some(ss) = SERVER_STATE.lock().await.get_mut(&self.server) {
            ss.velocity = velocity;
            ss.date_time = now_str();
        }
    }
}

impl ServerControl {
    pub(crate) fn new(server: String, rx: oneshot::Receiver<u8>) -> Self {
        ServerControl {
            server,
            rx: vec![rx],
            ip_scope: Vec::new(),
        }
    }

    pub(crate) fn get_scope(&mut self) -> &mut Vec<String> {
        &mut self.ip_scope
    }
}

async fn add_state(ss: ServerState) {
    SERVER_STATE.lock().await.insert((&ss.server).clone(), ss);
}

async fn remove_state(server: &String) -> Option<ServerState> {
    SERVER_STATE.lock().await.remove(server)
}

pub(crate) async fn hold(server: String) -> ServerControl {
    let (tx, rx) = oneshot::channel();
    add_state(ServerState::new(server.clone(), tx)).await;
    ServerControl::new(server, rx)
}

pub(crate) async fn list() -> String {
    let map = SERVER_STATE.lock().await;
    let mut str = String::new();
    for (key, val) in map.iter() {
        str.push_str(key);
        str.push_str(",velocity:");
        str.push_str(&val.velocity.to_string());
        str.push_str("c/s");
        if val.date_time.len() > 0 {
            str.push('[');
            str.push_str(&val.date_time);
            str.push(']');
        }
        str.push(' ');
    }
    str.pop();
    str
}

pub(crate) async fn state_string(server: &String) -> String {
    let map = SERVER_STATE.lock().await;
    let mut str = String::new();
    if let Some(ss) = map.get(server) {
        str.push_str(&ss.server);
        str.push_str(",velocity:");
        str.push_str(&ss.velocity.to_string());
    }
    str
}

//find signal sender and send stop signal to server
pub(crate) async fn shutdown(server: &String) -> bool {
    if let Some(s) = remove_state(server).await {
        match s.tx.send(0) {
            Ok(_) => return true,
            Err(e) => error!("Sender<u8> error: {:?}", e),
        }
    }
    false
}

pub(crate) async fn no_hold() -> (oneshot::Sender<u8>, ServerControl) {
    let (tx, rx) = oneshot::channel();
    (tx, ServerControl::new(String::new(), rx))
}
