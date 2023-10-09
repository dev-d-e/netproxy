pub(crate) use clap::Parser;
use std::net::SocketAddr;
use std::process::Command;

pub(crate) const CFGTOOL: &str = "on";

pub(crate) const SOCKETSAFE: &str = "off";

/// Arguments
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Args {
    /// The specified socket to receive configuration
    #[arg(short, long, default_value_t = SocketAddr::from(([127, 0, 0, 1], 0)))]
    pub(crate) socket: SocketAddr,

    /// Configuration should be from allowable ips. Scope is "localhost" when it's empty
    #[arg(long)]
    pub(crate) ipscope: Option<Vec<String>>,

    /// Use cfg tool if = "on"
    #[arg(long)]
    pub(crate) cfgtool: Option<String>,

    /// No use safe connection to socket if = "off"
    #[arg(long)]
    pub(crate) socsafe: Option<String>,
}

pub(crate) fn use_cfgtool(addr: SocketAddr) {
    let _process = match Command::new("./cfgtool")
        .arg("-t")
        .arg(addr.to_string())
        .spawn()
    {
        Ok(process) => process,
        Err(e) => panic!("fail to spawn 'cfgtool': {}", e),
    };
}
