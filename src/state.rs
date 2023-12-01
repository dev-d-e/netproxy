use lazy_static::lazy_static;
use log::error;
use std::collections::HashMap;
use tokio::sync::oneshot::Sender;
use tokio::sync::Mutex;

lazy_static! {
    static ref SERVER_STATE: Mutex<HashMap<String, ServerState>> = Mutex::new(HashMap::new());
}

#[derive(Debug)]
struct ServerState {
    server: String,
    velocity: f32,
    tx: Sender<u8>,
}

async fn add_state(ss: ServerState) {
    SERVER_STATE.lock().await.insert((&ss.server).clone(), ss);
}

async fn remove_state(server: &String) -> Option<ServerState> {
    SERVER_STATE.lock().await.remove(server)
}

pub(crate) async fn hold(server: String, tx: Sender<u8>) {
    let ss = ServerState {
        server,
        velocity: 0.0,
        tx,
    };
    add_state(ss).await;
}

pub(crate) async fn list() -> String {
    let map = SERVER_STATE.lock().await;
    let mut str = String::new();
    for (key, val) in map.iter() {
        str.push_str(key);
        str.push_str(",velocity:");
        str.push_str(&val.velocity.to_string());
        str.push_str(" ");
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
