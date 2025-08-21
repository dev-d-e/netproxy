use getset::{CopyGetters, MutGetters};
use log::{debug, error, trace};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const CAPACITY: usize = 8192;

#[derive(CopyGetters, MutGetters)]
pub(crate) struct BufStream<T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    stream: T,
    #[getset(get_mut = "pub(crate)")]
    r_buf: Vec<u8>,
    #[getset(get_mut = "pub(crate)")]
    w_buf: Vec<u8>,
    #[getset(get_copy = "pub(crate)")]
    r_f: bool,
    #[getset(get_copy = "pub(crate)")]
    w_f: bool,
}

impl<T> BufStream<T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    pub(crate) fn new(stream: T) -> Self {
        Self {
            stream,
            r_buf: Vec::with_capacity(CAPACITY),
            w_buf: Vec::with_capacity(CAPACITY),
            r_f: false,
            w_f: false,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.r_buf.is_empty()
    }

    pub(crate) fn has_remaining(&self) -> bool {
        self.r_buf.len() > 0
    }

    pub(crate) async fn read(&mut self) {
        match self.stream.read_buf(&mut self.r_buf).await {
            Ok(0) => {
                debug!("read 0 bytes");
                self.r_f = true;
            }
            Ok(n) => {
                trace!("read {} bytes", n);
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    debug!("read WouldBlock");
                } else {
                    error!("read error: {:?}", e);
                    self.r_f = true;
                }
            }
        }
    }

    pub(crate) async fn write(&mut self, o: Vec<u8>) {
        if let Err(e) = self.stream.write_all(&o).await {
            error!("write error: {:?}", e);
            self.w_f = true;
        }
    }

    pub(crate) fn take_buf(&mut self) -> Vec<u8> {
        std::mem::replace(&mut self.r_buf, Vec::with_capacity(CAPACITY))
    }

    pub(crate) async fn write_buf(&mut self) {
        if self.w_buf.len() > 0 {
            if let Err(e) = self.stream.write_all(self.w_buf.drain(..).as_slice()).await {
                error!("write error: {:?}", e);
                self.w_f = true;
            }
        }
    }
}
