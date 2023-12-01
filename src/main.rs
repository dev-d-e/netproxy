#![allow(dead_code)]

mod args;
mod builder;
mod core;
mod route;
mod state;
mod visit;

///It's a fast and convenient network proxy.
///Route data according to some configuration sentences.
///
///start up : 'netproxy'
///
pub fn main() {
    env_logger::builder().init();

    let args = args::init();

    builder::build(args);
}
