use log::{debug, error, info};
use std::future::Future;
use std::thread;
use std::time::Duration;
use tokio::runtime::Builder;

pub(crate) fn tokiort_block_on<T>(function: T)
where
    T: Future,
{
    let rt = match Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            error!("Builder error:{:?}", e);
            return;
        }
    };

    rt.block_on(function);

    rt.shutdown_background();
    info!("runtime shutdown");
}

pub(crate) fn new_thread_tokiort_block_on<T>(function: T)
where
    T: Future + Send + 'static,
{
    thread::spawn(move || {
        debug!("{:?} new thread", std::thread::current().id());
        let rt = match Builder::new_multi_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                error!("new thread Builder error:{:?}", e);
                return;
            }
        };

        rt.block_on(function);

        rt.shutdown_timeout(Duration::from_secs(1));
        info!("thread runtime shutdown");
    });
}
