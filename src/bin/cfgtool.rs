use clap::Parser;
use std::net::SocketAddr;
use tokio::io::{self, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let mut socket = TcpStream::connect(args.to).await.unwrap();

    let mut out = io::stdout();
    out.write_all(b"Please input configuration.\n")
        .await
        .unwrap();

    let mut ireader = BufReader::new(io::stdin());

    loop {
        let mut ibuf = String::new();
        let n = ireader.read_line(&mut ibuf).await.unwrap();
        if n > 1 {
            ibuf.remove(n - 1);
        } else {
            break;
        }
        if END_FLAG == ibuf.as_str() {
            socket.shutdown().await.unwrap();
            out.write_all(b"cfgtool closed.\n").await.unwrap();
            return;
        }
        socket.write_all(ibuf.as_bytes()).await.unwrap();

        let mut tbuf = [0; 1024];
        let n = socket.read(&mut tbuf).await.unwrap();
        let rsp = String::from_utf8(tbuf[0..n].to_vec()).unwrap();
        out.write_all(rsp.as_bytes()).await.unwrap();
        out.write_all(b"\n").await.unwrap();
        out.flush().await.unwrap();
    }
}

const END_FLAG: &str = ":!";

/// Arguments
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The socket to send configuration
    #[arg(short, long)]
    pub to: SocketAddr,
}
