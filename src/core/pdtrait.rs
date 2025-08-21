use async_trait::async_trait;

#[async_trait]
pub(crate) trait FuncR: Clone + Send + Sync + 'static {
    async fn data(&mut self, buf: &mut Vec<u8>);

    async fn enddata(&mut self, buf: &mut Vec<u8>);
}

#[async_trait]
pub(crate) trait FuncRw: Clone + Send + Sync + 'static {
    async fn service(&mut self, r_buf: &mut Vec<u8>, w_buf: &mut Vec<u8>);
}
