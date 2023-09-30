mod args;
mod builder;

use args::{Args, Parser};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let listener = TcpListener::bind(args.socket).await.unwrap();

    let addr = listener.local_addr().unwrap();

    if args::CFGTOOL == args.cfgtool.unwrap_or_default() {
        std::thread::spawn(move || args::use_cfgtool(addr));
    }

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        tokio::spawn(async move { builder::build(socket).await });
    }
}
