use clap::Parser;
use lazy_static::lazy_static;
use log::error;
use std::process::{Child, Command};
use std::sync::Mutex;

const YES: &str = "yes";

const NO: &str = "no";

lazy_static! {
    static ref CHILD: Mutex<Vec<Child>> = Mutex::new(Vec::new());
}

fn add_child(c: Child) {
    if let Ok(mut v) = CHILD.lock() {
        v.push(c);
    }
}

fn get_child() -> Option<Child> {
    if let Ok(mut v) = CHILD.lock() {
        v.pop()
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
    pub(crate) fn socket(&self) -> &String {
        &self.socket
    }

    pub(crate) fn is_tool(&self) -> bool {
        YES == self.cfgtool
    }

    pub(crate) fn is_safe(&self) -> bool {
        NO != self.socsafe
    }

    pub(crate) fn ipscope(&self) -> Vec<String> {
        self.ipscope.split(',').map(|s| s.to_string()).collect()
    }
}

pub(crate) fn init() -> Args {
    Args::parse()
}

pub(crate) fn spawn_tool(s: String, safe: bool) {
    if safe {
        std::thread::spawn(move || {
            match Command::new("./cfgtool")
                .arg("-t")
                .arg(s)
                .arg("--socsafe")
                .arg(YES)
                .spawn()
            {
                Ok(process) => add_child(process),
                Err(e) => error!("fail to spawn 'cfgtool': {}", e),
            };
        });
    } else {
        std::thread::spawn(move || {
            match Command::new("./cfgtool")
                .arg("-t")
                .arg(s)
                .arg("--socsafe")
                .arg(NO)
                .spawn()
            {
                Ok(process) => add_child(process),
                Err(e) => error!("fail to spawn 'cfgtool': {}", e),
            };
        });
    }
}

pub(crate) fn close_tool() {
    if let Some(mut c) = get_child() {
        if let Ok(_) = c.kill() {}
    }
}
