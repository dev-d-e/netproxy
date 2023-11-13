use super::{connect, connect_tls, mpsc_pair, FuncR, FuncRemote, FuncRw, FuncStream, Protoc};
use crate::{async_ext_read, async_ext_rw, tcp_stream_read, tcp_stream_start};
use async_trait::async_trait;
use log::{debug, error, trace, warn};
use std::fmt::Debug;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_native_tls::TlsStream;

#[derive(Debug)]
pub(crate) struct Procedure<T, S>(T, S, S)
where
    T: FuncRemote,
    S: FuncR;

impl<T, S> Clone for Procedure<T, S>
where
    T: FuncRemote,
    S: FuncR,
{
    fn clone(&self) -> Self {
        Procedure::new(self.0.clone(), self.1.clone(), self.2.clone())
    }
}

#[async_trait]
impl<T, S> FuncStream for Procedure<T, S>
where
    T: FuncRemote,
    S: FuncR,
{
    async fn tcp(self, server: TcpStream) {
        trace!("Procedure tcp start");
        let mut buf = Vec::<u8>::with_capacity(1024);
        let mut state: (bool, bool, Option<std::io::Error>) = (false, false, None);
        let n = 128;
        tcp_stream_start!(server, buf, n, state);
        if state.0 {
            error!("Procedure tcp error:{:?}", state.2);
            return;
        }
        if buf.len() == 0 {
            return;
        }
        let (protoc, remote, _n) = match self.0.get(&mut buf).await {
            Some(r) => (r.0, r.1, r.2),
            None => {
                warn!("no remote");
                return;
            }
        };
        debug!("remote[{:?}]", remote);
        if protoc == Protoc::TCP {
            tcp_to_tcp(server, buf, remote, self.1, self.2).await;
        } else {
            tcp_to_tls(server, buf, remote, self.1, self.2).await;
        }
    }

    async fn tls(self, server: TlsStream<TcpStream>) {
        trace!("Procedure tls start");
        let mut buf = Vec::<u8>::with_capacity(1024);
        let (protoc, remote, _n) = match self.0.get(&mut buf).await {
            Some(r) => (r.0, r.1, r.2),
            None => return,
        };
        debug!("remote[{:?}]", remote);
        if protoc == Protoc::TCP {
            tls_to_tcp(server, buf, remote, self.1, self.2).await;
        } else {
            tls_to_tls(server, buf, remote, self.1, self.2).await;
        }
    }
}

impl<T, S> Procedure<T, S>
where
    T: FuncRemote,
    S: FuncR,
{
    pub(crate) fn new(remote: T, server_data_func: S, remote_data_func: S) -> Procedure<T, S> {
        Procedure(remote, server_data_func, remote_data_func)
    }
}

#[derive(Debug)]
pub(crate) struct ProcedureService<T: FuncRw>(T);

impl<T: FuncRw> Clone for ProcedureService<T> {
    fn clone(&self) -> Self {
        ProcedureService::new(self.0.clone())
    }
}

#[async_trait]
impl<T: FuncRw> FuncStream for ProcedureService<T> {
    async fn tcp(self, server: TcpStream) {
        trace!("Service tcp start");
        handle(server, self.0).await;
    }

    async fn tls(self, server: TlsStream<TcpStream>) {
        trace!("Service tls start");
        handle(server, self.0).await;
    }
}

impl<T: FuncRw> ProcedureService<T> {
    pub(crate) fn new(func: T) -> ProcedureService<T> {
        ProcedureService(func)
    }
}

///
///====================================================================================================

async fn tcp_to_tcp(
    server: TcpStream,
    buf: Vec<u8>,
    remote: String,
    server_data_func: impl FuncR,
    remote_data_func: impl FuncR,
) {
    trace!("tcp_to_tcp");

    let mut remote = match connect(&remote).await {
        Ok(r) => r,
        Err(e) => {
            error!("connect error:{:?}", e);
            return;
        }
    };

    if let Err(e) = remote.write_all(&buf).await {
        error!("remote write error:{:?}", e);
        return;
    }

    let (rd, mut wr) = server.into_split();
    let (rd2, mut wr2) = remote.into_split();

    tokio::spawn(async move {
        loop {
            let mut buf = Vec::<u8>::with_capacity(10240);
            let mut state: (bool, bool, bool, Option<std::io::Error>) = (false, false, false, None);
            tcp_stream_read!(rd, buf, server_data_func, wr2, state);
            debug!("server->remote:{:?}", state.0 || state.2);
            if let Some(e) = state.3 {
                error!("server->remote error:{:?}", e);
            }
            if state.0 || state.2 {
                return;
            }
        }
    });

    tokio::spawn(async move {
        loop {
            let mut buf = Vec::<u8>::with_capacity(10240);
            let mut state: (bool, bool, bool, Option<std::io::Error>) = (false, false, false, None);
            tcp_stream_read!(rd2, buf, remote_data_func, wr, state);
            debug!("remote->server:{:?}", state.0 || state.2);
            if let Some(e) = state.3 {
                error!("remote->server error:{:?}", e);
            }
            if state.0 || state.2 {
                return;
            }
        }
    });
}

async fn tcp_to_tls(
    server: TcpStream,
    buf: Vec<u8>,
    remote: String,
    server_data_func: impl FuncR,
    remote_data_func: impl FuncR,
) {
    trace!("tcp_to_tls");

    let mut remote = match connect_tls(&remote).await {
        Ok(r) => r,
        Err(e) => {
            error!("connect error:{:?}", e);
            return;
        }
    };

    if let Err(e) = remote.write_all(&buf).await {
        error!("remote write error:{:?}", e);
        return;
    }

    let (rd, mut wr) = server.into_split();

    let (tx1, mut rx1, tx2, mut rx2) = mpsc_pair::<(TlsStream<TcpStream>, bool)>();
    if let Err(e) = tx1.send((remote, false)).await {
        error!("mpsc::channel error:{:?}", e);
        return;
    };

    tokio::spawn(async move {
        loop {
            let (mut remote, closed) = match rx1.recv().await {
                Some(s) => s,
                None => return,
            };

            let mut buf = Vec::<u8>::with_capacity(10240);
            let mut state: (bool, bool, bool, Option<std::io::Error>) = (false, false, false, None);
            tcp_stream_read!(rd, buf, server_data_func, remote, state);
            debug!("server->remote:{:?}", state.0 || state.2);
            if closed {
                return;
            }
            if let Err(e) = tx2.send((remote, state.2)).await {
                error!("mpsc::channel error:{:?}", e);
                return;
            }
            if let Some(e) = state.3 {
                error!("server->remote error:{:?}", e);
            }
            if state.0 || state.2 {
                return;
            }
            trace!("server->remote: done.");
        }
    });

    tokio::spawn(async move {
        loop {
            let (mut remote, closed) = match rx2.recv().await {
                Some(s) => s,
                None => return,
            };
            let mut buf = Vec::<u8>::with_capacity(10240);
            let mut state: (bool, bool, bool, Option<std::io::Error>) = (false, false, false, None);
            async_ext_read!(remote, buf, remote_data_func, wr, state);
            debug!("remote->server:{:?}", state.0 || state.2);
            if closed {
                return;
            }
            if let Err(e) = tx1.send((remote, state.0)).await {
                error!("mpsc::channel error:{:?}", e);
                return;
            };
            if let Some(e) = state.3 {
                error!("remote->server error:{:?}", e);
            }
            if state.0 || state.2 {
                return;
            }
            trace!("remote->server: done.");
        }
    });
}

async fn tls_to_tcp(
    server: TlsStream<TcpStream>,
    buf: Vec<u8>,
    remote: String,
    server_data_func: impl FuncR,
    remote_data_func: impl FuncR,
) {
    trace!("tls_to_tcp");

    let mut remote = match connect(&remote).await {
        Ok(r) => r,
        Err(e) => {
            error!("connect error:{:?}", e);
            return;
        }
    };

    if let Err(e) = remote.write_all(&buf).await {
        error!("remote write error:{:?}", e);
        return;
    }

    let (tx1, mut rx1, tx2, mut rx2) = mpsc_pair::<(TlsStream<TcpStream>, bool)>();
    if let Err(e) = tx1.send((server, false)).await {
        error!("mpsc::channel error:{:?}", e);
        return;
    };

    let (mut rd, mut wr) = remote.into_split();

    tokio::spawn(async move {
        loop {
            let (mut server, closed) = match rx1.recv().await {
                Some(s) => s,
                None => return,
            };

            let mut buf = Vec::<u8>::with_capacity(10240);
            let mut state: (bool, bool, bool, Option<std::io::Error>) = (false, false, false, None);
            async_ext_read!(server, buf, server_data_func, wr, state);
            debug!("server->remote:{:?}", state.0 || state.2);
            if closed {
                return;
            }
            if let Err(e) = tx2.send((server, state.0)).await {
                error!("mpsc::channel error:{:?}", e);
                return;
            };
            if let Some(e) = state.3 {
                error!("server->remote error:{:?}", e);
            }
            if state.0 || state.2 {
                return;
            }
            trace!("server->remote: done.");
        }
    });

    tokio::spawn(async move {
        loop {
            let (mut server, closed) = match rx2.recv().await {
                Some(s) => s,
                None => return,
            };

            let mut buf = Vec::<u8>::with_capacity(10240);
            let mut state: (bool, bool, bool, Option<std::io::Error>) = (false, false, false, None);
            async_ext_read!(rd, buf, remote_data_func, server, state);
            debug!("remote->server:{:?}", state.0 || state.2);
            if closed {
                return;
            }
            if let Err(e) = tx1.send((server, state.2)).await {
                error!("mpsc::channel error:{:?}", e);
                return;
            };
            if let Some(e) = state.3 {
                error!("remote->server error:{:?}", e);
            }
            if state.0 || state.2 {
                return;
            }
            trace!("remote->server: done.");
        }
    });
}

async fn tls_to_tls(
    server: TlsStream<TcpStream>,
    buf: Vec<u8>,
    remote: String,
    server_data_func: impl FuncR,
    remote_data_func: impl FuncR,
) {
    trace!("tls_to_tls");

    let mut remote = match connect_tls(&remote).await {
        Ok(r) => r,
        Err(e) => {
            error!("connect error:{:?}", e);
            return;
        }
    };

    if let Err(e) = remote.write_all(&buf).await {
        error!("remote write error:{:?}", e);
        return;
    }

    let (tx1, mut rx1, tx2, mut rx2) =
        mpsc_pair::<(TlsStream<TcpStream>, bool, TlsStream<TcpStream>, bool)>();
    if let Err(e) = tx1.send((server, false, remote, false)).await {
        error!("mpsc::channel error:{:?}", e);
        return;
    };

    tokio::spawn(async move {
        loop {
            let (mut server, s_closed, mut remote, r_closed) = match rx1.recv().await {
                Some(s) => s,
                None => return,
            };

            let mut buf = Vec::<u8>::with_capacity(10240);
            let mut state: (bool, bool, bool, Option<std::io::Error>) = (false, false, false, None);
            async_ext_read!(server, buf, server_data_func, remote, state);
            debug!("server->remote:{:?}", state.0 || state.2);
            if r_closed {
                return;
            }
            if let Err(e) = tx2.send((server, state.0, remote, state.2)).await {
                error!("mpsc::channel error:{:?}", e);
                return;
            };
            if let Some(e) = state.3 {
                error!("server->remote error:{:?}", e);
            }
            if s_closed || r_closed || state.0 || state.2 {
                return;
            }
            trace!("server->remote: done.");
        }
    });

    tokio::spawn(async move {
        loop {
            let (mut server, s_closed, mut remote, r_closed) = match rx2.recv().await {
                Some(s) => s,
                None => return,
            };

            let mut buf = Vec::<u8>::with_capacity(10240);
            let mut state: (bool, bool, bool, Option<std::io::Error>) = (false, false, false, None);
            async_ext_read!(remote, buf, remote_data_func, server, state);
            debug!("remote->server:{:?}", state.0 || state.2);
            if s_closed {
                return;
            }
            if let Err(e) = tx1.send((server, state.2, remote, state.0)).await {
                error!("mpsc::channel error:{:?}", e);
                return;
            };
            if let Some(e) = state.3 {
                error!("remote->server error:{:?}", e);
            }
            if s_closed || r_closed || state.0 || state.2 {
                return;
            }
            trace!("remote->server: done.");
        }
    });
}

async fn handle<S: AsyncReadExt + AsyncWriteExt + Unpin>(mut server: S, func: impl FuncRw) {
    loop {
        let func = func.clone();

        let mut r_buf = Vec::<u8>::with_capacity(10240);
        let mut w_buf = Vec::<u8>::with_capacity(10240);
        let mut state: (bool, bool, bool, Option<std::io::Error>) = (false, false, false, None);
        async_ext_rw!(server, r_buf, w_buf, func, state);
        if let Some(e) = state.3 {
            error!("Service error:{:?}", e);
        }
        if state.0 || state.2 {
            return;
        }
        trace!("Service: done.");
    }
}
