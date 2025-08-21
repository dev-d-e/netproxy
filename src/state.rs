use crate::core::*;
use getset::{CopyGetters, Getters, MutGetters, Setters};
use log::error;
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use tokio::sync::{mpsc, oneshot, Mutex, OnceCell};

static SERVER_STATE: OnceCell<Mutex<HashMap<SocketAddr, ServerState>>> = OnceCell::const_new();

async fn server_state() -> &'static Mutex<HashMap<SocketAddr, ServerState>> {
    SERVER_STATE
        .get_or_init(|| async { Mutex::new(HashMap::new()) })
        .await
}

#[derive(CopyGetters, Getters, MutGetters, Setters)]
pub(crate) struct ServerState {
    server: SocketAddr,
    c: oneshot::Sender<ControlInfo>,
    #[getset(get_copy = "pub(crate)", set = "pub(crate)")]
    velocity: u32,
    #[getset(get = "pub(crate)", get_mut = "pub(crate)")]
    date_time: String,
    ip_scope: HashSet<IpAddr>,
}

impl ServerState {
    pub(crate) fn new(server: SocketAddr, c: oneshot::Sender<ControlInfo>) -> Self {
        Self {
            server,
            c,
            velocity: 0,
            date_time: String::new(),
            ip_scope: HashSet::new(),
        }
    }

    pub(crate) fn send(self, o: ControlInfo) -> Result<(), ControlInfo> {
        self.c.send(o)
    }
}

async fn add_state(ss: ServerState) {
    server_state()
        .await
        .lock()
        .await
        .insert((&ss.server).clone(), ss);
}

async fn remove_state(server: &SocketAddr) -> Option<ServerState> {
    server_state().await.lock().await.remove(server)
}

pub(crate) async fn hold(
    server: SocketAddr,
    c: oneshot::Sender<ControlInfo>,
    mut s: mpsc::Receiver<StateInfo>,
) {
    add_state(ServerState::new(server.clone(), c)).await;

    tokio::spawn(async move {
        while let Some(o) = s.recv().await {
            match o {
                StateInfo::Sum(velocity, date_time) => {
                    if let Some(ss) = server_state().await.lock().await.get_mut(&server) {
                        ss.set_velocity(velocity);
                        ss.date_time = date_time;
                    }
                }
            }
        }
    });
}

pub(crate) async fn server_accept(addr: &str, func: impl FuncStream) {
    if let Ok((mut server, a, b)) = Server::new(addr)
        .await
        .inspect_err(|e| error!("new server: {:?}", e))
    {
        hold(*server.addr(), a, b).await;
        server.accept(func).await;
    }
}

pub(crate) async fn list() -> String {
    let map = server_state().await.lock().await;
    let mut s = String::new();
    s.push_str("ok ");
    for (key, val) in map.iter() {
        s.push_str(&key.to_string());
        s.push_str(",velocity:");
        s.push_str(&val.velocity.to_string());
        if val.date_time.len() > 0 {
            s.push(' ');
            s.push('[');
            s.push_str(&val.date_time);
            s.push(']');
        }
        s.push(' ');
    }
    s.pop();
    s
}

pub(crate) async fn state_string(server: &SocketAddr) -> String {
    let map = server_state().await.lock().await;
    let mut s = String::new();
    s.push_str("ok ");
    if let Some(ss) = map.get(server) {
        s.push_str(&ss.server.to_string());
        s.push_str(",velocity:");
        s.push_str(&ss.velocity.to_string());
        if ss.date_time.len() > 0 {
            s.push(' ');
            s.push('[');
            s.push_str(&ss.date_time);
            s.push(']');
        }
    }
    s
}

//find signal sender and send stop signal to server
pub(crate) async fn shutdown(server: &SocketAddr) -> bool {
    remove_state(server)
        .await
        .map(|s| {
            s.send(ControlInfo::Close)
                .inspect_err(|_| error!("shutdown error"))
                .ok()
        })
        .flatten()
        .is_some()
}
