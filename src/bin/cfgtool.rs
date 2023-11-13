use clap::Parser;
use std::fmt::Debug;
use tokio::io::{
    self, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};
use tokio::net::TcpStream;
use tokio_native_tls::native_tls::TlsConnector as NativeTlsConnector;
use tokio_native_tls::TlsConnector;

const END_FLAG: &str = ":!";

const NO: &str = "no";

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if args.is_safe() {
        connsafe(&args).await;
    } else {
        conn(&args).await;
    }
}

async fn connsafe(args: &Args) -> bool {
    let socket = TcpStream::connect(&args.to).await.unwrap();
    let c = NativeTlsConnector::new().unwrap();
    let c = TlsConnector::from(c);
    let mut s = args.to.to_string();
    if let Some(n) = args.to.find(':') {
        s.truncate(n);
    }
    match c.connect(s.as_str(), socket).await {
        Ok(socket) => {
            input(socket).await;
            return true;
        }
        Err(e) => {
            println!("{:?}", e);
            return false;
        }
    };
}

async fn conn(args: &Args) -> bool {
    let socket = TcpStream::connect(&args.to).await.unwrap();
    input(socket).await;
    true
}

async fn input<T>(mut socket: T)
where
    T: AsyncRead + AsyncWrite + AsyncWriteExt + Unpin + Debug,
{
    println!("{:?}", socket);
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

/// Arguments
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The socket to send configuration
    #[arg(short, long)]
    pub to: String,

    /// Whether to use safe connection to socket (yes/no)
    #[arg(long, default_value = NO)]
    socsafe: String,
}

impl Args {
    fn is_safe(&self) -> bool {
        NO != self.socsafe
    }
}
