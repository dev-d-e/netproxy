mod cert;
mod hr;
mod pd;
mod pdtrait;
mod rt;
mod rw;
mod sv;

pub(crate) use cert::get_identity;
pub(crate) use hr::parse_request;
use log::trace;
pub(crate) use pd::{Procedure, ProcedureService};
pub(crate) use pdtrait::{FuncR, FuncRemote, FuncRw};
pub(crate) use rt::{new_thread_tokiort_block_on, tokiort_block_on};
pub(crate) use sv::{connect, connect_tls, FuncStream, Server};
use tokio::sync::mpsc::{self, Receiver, Sender};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Protoc {
    TCP,
    TLS,
    UDP,
    HTTP,
    HTTPPT,
}

pub(crate) fn mpsc_pair<T>() -> (Sender<T>, Receiver<T>, Sender<T>, Receiver<T>) {
    let (tx1, rx1) = mpsc::channel::<T>(1);
    let (tx2, rx2) = mpsc::channel::<T>(1);
    (tx1, rx1, tx2, rx2)
}

struct StrWrapper(String);

impl utf8parse::Receiver for StrWrapper {
    fn codepoint(&mut self, c: char) {
        self.0.push(c);
    }

    fn invalid_sequence(&mut self) {}
}

pub(crate) fn into_str(buf: &mut Vec<u8>) -> String {
    trace!("into_str:{:?}", buf.len());
    let mut p = utf8parse::Parser::new();
    let mut t = StrWrapper(String::new());
    for byte in buf.into_iter() {
        p.advance(&mut t, *byte);
    }
    t.0
}
