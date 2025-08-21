use super::*;
use async_trait::async_trait;
use log::{debug, trace};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_native_tls::TlsAcceptor;

pub(crate) async fn read_loop<T, S>(
    server: &mut BufStream<T>,
    remote: &mut BufStream<S>,
    server_data_func: &mut impl FuncR,
    remote_data_func: &mut impl FuncR,
) where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    if server.has_remaining() {
        let mut v = server.take_buf();
        server_data_func.data(&mut v).await;
        remote.write(v).await;
    }
    loop {
        tokio::select! {
            _ = server.read() => {
                if server.has_remaining(){
                    let mut v = server.take_buf();
                    if server.r_f(){
                        server_data_func.enddata(&mut v).await;
                    }else{
                        server_data_func.data(&mut v).await;
                    }
                    remote.write(v).await;
                }
                if server.r_f() || remote.w_f() {
                    debug!("server->remote end");
                    return;
                }
            }
            _ = remote.read() => {
                if remote.has_remaining(){
                    let mut v = remote.take_buf();
                    if remote.r_f() {
                        remote_data_func.enddata(&mut v).await;
                    }else{
                        remote_data_func.data(&mut v).await;
                    }
                    server.write(v).await;
                }
                if remote.r_f() || server.w_f() {
                    debug!("remote->server end");
                    return;
                }
            }
        }
    }
}

async fn service_loop<T, S>(mut server: BufStream<T>, mut func: S)
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
    S: FuncRw,
{
    loop {
        server.read().await;
        if server.has_remaining() {
            let mut r_buf = server.take_buf();
            func.service(&mut r_buf, server.w_buf_mut()).await;
            server.write_buf().await;
        }
        if server.r_f() || server.w_f() {
            debug!("end");
            break;
        }
    }
}

#[derive(Clone)]
pub(crate) struct ServiceTls<T>
where
    T: FuncRw,
{
    func: T,
    tls_acceptor: Arc<TlsAcceptor>,
}

#[async_trait]
impl<T> FuncStream for ServiceTls<T>
where
    T: FuncRw,
{
    async fn consume(self, server: TcpStream) {
        trace!("service tls start");
        if let Ok(server) = tls_accept(&self.tls_acceptor, server).await {
            service_loop(BufStream::new(server), self.func).await;
        }
    }
}

impl<T> ServiceTls<T>
where
    T: FuncRw,
{
    pub(crate) fn new(func: T, t: TlsAcceptor) -> Self {
        Self {
            func,
            tls_acceptor: Arc::new(t),
        }
    }
}

#[derive(Clone)]
pub(crate) struct Service<T>
where
    T: FuncRw,
{
    func: T,
}

#[async_trait]
impl<T> FuncStream for Service<T>
where
    T: FuncRw,
{
    async fn consume(mut self, server: TcpStream) {
        trace!("service start");
        service_loop(BufStream::new(server), self.func).await;
    }
}

impl<T> Service<T>
where
    T: FuncRw,
{
    pub(crate) fn new(func: T) -> Self {
        Self { func }
    }
}
