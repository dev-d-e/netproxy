use super::*;
use std::future::Future;
use std::thread;
use std::time::Duration;
use tokio::runtime::Builder;

pub(crate) fn tokiort_block_on<T>(function: T)
where
    T: Future,
{
    let rt = if let Ok(rt) = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .map_err(|e| error!("tokiort_block_on: {e}"))
    {
        rt
    } else {
        return;
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
        let rt = if let Ok(rt) = Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| error!("new_thread_tokiort_block_on: {e}"))
        {
            rt
        } else {
            return;
        };

        rt.block_on(function);

        rt.shutdown_timeout(Duration::from_secs(1));
        info!("thread runtime shutdown");
    });
}
