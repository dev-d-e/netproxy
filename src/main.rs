mod args;
mod builder;
mod core;
mod route;
mod visit;

use args::{Args, Parser};

///It's a fast and convenient network proxy.
///Route data according to some configuration sentences.
///
///start up : 'netproxy'
///
pub fn main() {
    env_logger::builder().init();

    let args = Args::parse();

    builder::build_server(&args.socket, args.is_safe(), |s| {
        args.check_tool(s);
    });
}
