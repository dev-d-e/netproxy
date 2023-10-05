use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug)]
struct Cfginfo {
    target: String,
    raddr: String,
    saddr: String,
}

impl Cfginfo {
    fn new(str: String) -> Result<Cfginfo, String> {
        let cv: Vec<&str> = str.split_whitespace().collect();
        if cv.len() == 3 {
            Ok(Cfginfo {
                target: cv[0].to_string(),
                raddr: cv[1].to_string(),
                saddr: cv[2].to_string(),
            })
        } else {
            panic!("fail to parse info")
        }
    }

    fn get_target(&self) -> String {
        self.target.to_string()
    }

    fn get_raddr(&self) -> String {
        self.raddr.to_string()
    }

    fn get_saddr(&self) -> String {
        self.saddr.to_string()
    }
}

pub(crate) async fn build(mut ts: TcpStream) {
    let mut buf = [0; 1024];

    loop {
        let n = ts.read(&mut buf).await.unwrap();
        if n == 0 {
            return;
        }
        let str = String::from_utf8(buf[0..n].to_vec()).unwrap();

        let ci = Cfginfo::new(str).unwrap();
        if "tcp" == ci.get_target() {
            tcp(ci.get_raddr(), ci.get_saddr());
        }

        ts.write_all(b"config success").await.unwrap();
    }
}

fn tcp(raddr: String, saddr: String) {
    tokio::spawn(async move {
        let rlistener = TcpListener::bind(raddr).await.unwrap();
        loop {
            let (mut rts, _) = rlistener.accept().await.unwrap();
            let saddr2 = saddr.clone();
            tokio::spawn(async move {
                let mut sts = TcpStream::connect(saddr2).await.unwrap();
                io::copy_bidirectional(&mut rts, &mut sts).await.unwrap();
            });
        }
    });
}
