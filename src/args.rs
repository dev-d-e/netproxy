use clap::Parser;
use log::error;
use std::net::{IpAddr, SocketAddr};
use std::process::{Child, Command};
use std::sync::{Mutex, OnceLock};

const YES: &str = "yes";

const NO: &str = "no";

static CHILD: OnceLock<Mutex<Option<Child>>> = OnceLock::new();

fn child_mut() -> &'static Mutex<Option<Child>> {
    CHILD.get_or_init(|| Mutex::new(None))
}

fn add_child(c: Child) {
    if let Ok(mut v) = child_mut().lock() {
        v.replace(c);
    }
}

fn get_child() -> Option<Child> {
    if let Ok(mut v) = child_mut().lock() {
        v.take()
    } else {
        None
    }
}

/// Arguments
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Args {
    /// The specified socket to receive configuration
    #[arg(short, long, default_value = "127.0.0.1:0")]
    socket: String,

    /// Configuration should be from allowable ips
    #[arg(long, default_value = "127.0.0.1,::1")]
    ipscope: String,

    /// Whether to use cfg tool (yes/no)
    #[arg(long, default_value = NO)]
    cfgtool: String,

    /// Whether to use safe connection to socket (yes/no)
    #[arg(long, default_value = NO)]
    socsafe: String,
}

impl Args {
    pub(crate) fn socket(&self) -> &str {
        &self.socket
    }

    pub(crate) fn is_tool(&self) -> bool {
        YES == self.cfgtool
    }

    pub(crate) fn is_safe(&self) -> bool {
        NO != self.socsafe
    }

    pub(crate) fn ipscope(&self) -> Vec<IpAddr> {
        self.ipscope
            .split(',')
            .filter_map(|s| s.parse::<IpAddr>().ok())
            .collect()
    }
}

pub(crate) fn init() -> Args {
    Args::parse()
}

pub(crate) fn spawn_tool(addr: &SocketAddr, safe: bool) {
    let s = addr.to_string();
    let socsafe = if safe { YES } else { NO };
    std::thread::spawn(move || {
        match Command::new("./cfgtool")
            .arg("-t")
            .arg(s)
            .arg("--socsafe")
            .arg(socsafe)
            .spawn()
        {
            Ok(process) => add_child(process),
            Err(e) => error!("fail to spawn 'cfgtool': {}", e),
        };
    });
}

pub(crate) fn close_tool() {
    if let Some(mut c) = get_child() {
        if let Ok(_) = c.kill() {}
    }
}
