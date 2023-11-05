use super::Protoc;
use async_trait::async_trait;
use std::marker::Send;

#[async_trait]
pub(crate) trait FuncRemote: Clone + Send + Sync + 'static {
    async fn get(&self, buf: &mut Vec<u8>) -> Option<(Protoc, String, usize)>;
}

#[async_trait]
pub(crate) trait FuncR: Clone + Send + Sync + 'static {
    async fn data(&self, buf: &mut Vec<u8>);

    async fn enddata(&self, buf: &mut Vec<u8>);
}

#[macro_export]
macro_rules! transfer_data {
    ($name:ident, $self:ident, $buf:ident, $func:stmt, $func2:stmt) => {
        #[derive(Copy, Clone, Debug)]
        struct $name;

        #[async_trait]
        impl FuncR for $name {
            async fn data(&$self, $buf: &mut Vec<u8>) {
                $func
            }

            async fn enddata(&$self, $buf: &mut Vec<u8>){
                $func2
            }
        }
    };
}

#[async_trait]
pub(crate) trait FuncRw: Clone + Send + Sync + 'static {
    async fn service(self, r_buf: &mut Vec<u8>, w_buf: &mut Vec<u8>);
}

#[macro_export]
macro_rules! rw_service {
    ($name:ident, $self:ident, $req:ident, $rsp:ident, $func:stmt) => {
        #[derive(Copy, Clone, Debug)]
        struct $name;

        #[async_trait]
        impl FuncRw for $name {
            async fn service($self, $req: &mut Vec<u8>, $rsp: &mut Vec<u8>) {
                $func
            }
        }
    };
}
