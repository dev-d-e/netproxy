use super::{connect, connect_tls, FuncR, FuncRemote, FuncRw, FuncStream, Protoc};
use async_trait::async_trait;
use log::{debug, error, trace, warn};
use std::fmt::Debug;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_native_tls::TlsStream;

const CAPACITY: usize = 8192;

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
    async fn tcp(mut self, server: TcpStream) {
        trace!("Procedure start");
        let mut buf = Vec::<u8>::with_capacity(CAPACITY);
        tcp_stream_start!(server, buf);
        if buf.len() == 0 {
            return;
        }
        let remote = match self.0.get(&mut buf).await {
            Some(r) => r,
            None => {
                warn!("no remote");
                return;
            }
        };
        debug!("remote[{:?}]", remote.host);
        if remote.protoc == Protoc::TCP {
            tcp_to_tcp(server, buf, remote.host, self.1, self.2).await;
        } else {
            tcp_to_tls(server, buf, remote.host, self.1, self.2).await;
        }
    }

    async fn tls(mut self, mut server: TlsStream<TcpStream>) {
        trace!("Procedure start");
        let mut buf = Vec::<u8>::with_capacity(CAPACITY);
        async_ext_start!(server, buf);
        if buf.len() == 0 {
            return;
        }
        let remote = match self.0.get(&mut buf).await {
            Some(r) => r,
            None => {
                warn!("no remote");
                return;
            }
        };
        debug!("remote[{:?}]", remote.host);
        if remote.protoc == Protoc::TCP {
            tls_to_tcp(server, buf, remote.host, self.1, self.2).await;
        } else {
            tls_to_tls(server, buf, remote.host, self.1, self.2).await;
        }
    }
}

impl<T, S> Procedure<T, S>
where
    T: FuncRemote,
    S: FuncR,
{
    pub(crate) fn new(remote: T, server_data_func: S, remote_data_func: S) -> Self {
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
        trace!("Service start");
        handle(server, self.0).await;
        trace!("Service end");
    }

    async fn tls(self, server: TlsStream<TcpStream>) {
        trace!("Service start");
        handle(server, self.0).await;
        trace!("Service end");
    }
}

impl<T: FuncRw> ProcedureService<T> {
    pub(crate) fn new(func: T) -> Self {
        ProcedureService(func)
    }
}

//
//====================================================================================================

async fn tcp_to_tcp(
    mut server: TcpStream,
    mut buf: Vec<u8>,
    remote: String,
    mut server_data_func: impl FuncR,
    mut remote_data_func: impl FuncR,
) {
    trace!("tcp_to_tcp");

    let mut remote = match connect(&remote).await {
        Ok(r) => r,
        Err(e) => {
            error!("remote error:{:?}", e);
            return;
        }
    };

    if let Err(e) = remote.write_all(&buf).await {
        error!("remote write error:{:?}", e);
        return;
    }

    let (rd, mut wr) = server.split();
    let (rd2, mut wr2) = remote.split();

    let mut buf2 = Vec::<u8>::with_capacity(CAPACITY);
    loop {
        tokio::select! {
            Ok(_) = rd.readable() => {
                let mut state: (bool, bool, bool) = (false, false, false);
                tcp_stream_read!(rd, buf, server_data_func, wr2, state, "server->remote");
                if state.0 || state.2 {
                    debug!("server->remote: end");
                    return;
                }
            }
            Ok(_) = rd2.readable() => {
                let mut state: (bool, bool, bool) = (false, false, false);
                tcp_stream_read!(rd2, buf2, remote_data_func, wr, state, "remote->server");
                if state.0 || state.2 {
                    debug!("remote->server: end");
                    return;
                }
            }
        }
    }
}

async fn tcp_to_tls(
    mut server: TcpStream,
    mut buf: Vec<u8>,
    remote: String,
    mut server_data_func: impl FuncR,
    mut remote_data_func: impl FuncR,
) {
    trace!("tcp_to_tls");

    let mut remote = match connect_tls(&remote).await {
        Ok(r) => r,
        Err(e) => {
            error!("remote error:{:?}", e);
            return;
        }
    };

    if let Err(e) = remote.write_all(&buf).await {
        error!("remote write error:{:?}", e);
        return;
    }

    let (rd, mut wr) = server.split();

    let mut buf2 = Vec::<u8>::with_capacity(CAPACITY);
    loop {
        tokio::select! {
            Ok(_) = rd.readable() => {
                let mut state: (bool, bool, bool) = (false, false, false);
                tcp_stream_read!(rd, buf, server_data_func, remote, state, "server->remote");
                if state.0 || state.2 {
                    debug!("server->remote: end");
                    return;
                }
            },
            r = remote.read_buf(&mut buf2) => {
                let mut state: (bool, bool, bool) = (false, false, false);
                async_ext_read!(r, buf2, remote_data_func, wr, state, "remote->server");
                if state.0 || state.2 {
                    debug!("remote->server: end");
                 return;
                }
            }
        }
    }
}

async fn tls_to_tcp(
    mut server: TlsStream<TcpStream>,
    mut buf: Vec<u8>,
    remote: String,
    mut server_data_func: impl FuncR,
    mut remote_data_func: impl FuncR,
) {
    trace!("tls_to_tcp");

    let mut remote = match connect(&remote).await {
        Ok(r) => r,
        Err(e) => {
            error!("remote error:{:?}", e);
            return;
        }
    };

    if let Err(e) = remote.write_all(&buf).await {
        error!("remote write error:{:?}", e);
        return;
    }

    let (rd, mut wr) = remote.split();

    let mut buf2 = Vec::<u8>::with_capacity(CAPACITY);
    loop {
        tokio::select! {
            r = server.read_buf(&mut buf) => {
                let mut state: (bool, bool, bool) = (false, false, false);
                async_ext_read!(r, buf, server_data_func, wr, state, "server->remote");
                if state.0 || state.2 {
                    debug!("server->remote: end");
                    return;
                }
            }
            Ok(_) = rd.readable() => {
                let mut state: (bool, bool, bool) = (false, false, false);
                tcp_stream_read!(rd, buf2, remote_data_func, server, state, "remote->server");
                if state.0 || state.2 {
                    debug!("remote->server: end");
                    return;
                }
            }
        }
    }
}

async fn tls_to_tls(
    mut server: TlsStream<TcpStream>,
    mut buf: Vec<u8>,
    remote: String,
    mut server_data_func: impl FuncR,
    mut remote_data_func: impl FuncR,
) {
    trace!("tls_to_tls");

    let mut remote = match connect_tls(&remote).await {
        Ok(r) => r,
        Err(e) => {
            error!("remote error:{:?}", e);
            return;
        }
    };

    if let Err(e) = remote.write_all(&buf).await {
        error!("remote write error:{:?}", e);
        return;
    }

    let mut buf2 = Vec::<u8>::with_capacity(CAPACITY);
    loop {
        tokio::select! {
            r = server.read_buf(&mut buf) => {
                let mut state: (bool, bool, bool) = (false, false, false);
                async_ext_read!(r, buf, server_data_func, remote, state, "server->remote");
                if state.0 || state.2 {
                    debug!("server->remote: end");
                    return;
                }
            }
            r = remote.read_buf(&mut buf2) => {
                let mut state: (bool, bool, bool) = (false, false, false);
                 async_ext_read!(r, buf2, remote_data_func, server, state, "remote->server");
                if state.0 || state.2 {
                    debug!("remote->server: end");
                    return;
                }
            }
        }
    }
}

async fn handle<S: AsyncReadExt + AsyncWriteExt + Unpin>(mut server: S, func: impl FuncRw) {
    loop {
        let mut func = func.clone();

        let mut r_buf = Vec::<u8>::with_capacity(CAPACITY);
        let mut w_buf = Vec::<u8>::with_capacity(CAPACITY);
        let mut state: (bool, bool, bool) = (false, false, false);
        async_ext_rw!(server, r_buf, w_buf, func, state);
        if state.0 || state.2 {
            return;
        }
    }
}
