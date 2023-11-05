pub(crate) use clap::Parser;
use std::process::Command;

const YES: &str = "yes";

const NO: &str = "no";

/// Arguments
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Args {
    /// The specified socket to receive configuration
    #[arg(short, long, default_value = "127.0.0.1:0")]
    pub(crate) socket: String,

    /// Configuration should be from allowable ips. Scope is "localhost" when it's empty
    #[arg(long)]
    pub(crate) ipscope: Option<Vec<String>>,

    /// Whether to use cfg tool (yes/no)
    #[arg(long, default_value = NO)]
    pub(crate) cfgtool: String,

    /// Whether to use safe connection to socket (yes/no)
    #[arg(long, default_value = NO)]
    pub(crate) socsafe: String,
}

impl Args {
    pub(crate) fn is_tool(&self) -> bool {
        YES == self.cfgtool
    }

    pub(crate) fn check_tool(&self, s: String) {
        if self.is_tool() {
            if self.is_safe() {
                std::thread::spawn(move || {
                    let _process = match Command::new("./cfgtool")
                        .arg("-t")
                        .arg(s)
                        .arg("--socsafe")
                        .arg(YES)
                        .spawn()
                    {
                        Ok(process) => process,
                        Err(e) => panic!("fail to spawn 'cfgtool': {}", e),
                    };
                });
            } else {
                std::thread::spawn(move || {
                    let _process = match Command::new("./cfgtool")
                        .arg("-t")
                        .arg(s)
                        .arg("--socsafe")
                        .arg(NO)
                        .spawn()
                    {
                        Ok(process) => process,
                        Err(e) => panic!("fail to spawn 'cfgtool': {}", e),
                    };
                });
            }
        }
    }

    pub(crate) fn is_safe(&self) -> bool {
        NO != self.socsafe
    }
}
