mod cert;
mod hr;
mod pd;
mod pdtrait;
mod rt;
mod rw;
mod sv;

pub(crate) use cert::{build_certificate_from_file, build_certificate_from_socket, valid_identity};
pub(crate) use hr::parse_request;
use log::trace;
pub(crate) use pd::{Procedure, ProcedureService};
pub(crate) use pdtrait::{FuncR, FuncRemote, FuncRw};
pub(crate) use rt::{new_thread_tokiort_block_on, tokiort_block_on};
use std::fs;
use std::path::PathBuf;
pub(crate) use sv::{connect, connect_tls, FuncControl, FuncStream, Server};
use time::{OffsetDateTime, UtcOffset};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Protoc {
    TCP,
    TLS,
    UDP,
    HTTP,
    HTTPPT,
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

///get file by "path".
///if it's a file path, read it to "Vec".
///if it's a dir path, read the first file in the dir to "Vec".
fn get_file(path: &str) -> std::io::Result<Vec<u8>> {
    let metadata = fs::metadata(path)?;
    if metadata.is_file() {
        trace!("get file:{:?}", path);
        return fs::read(path);
    } else if metadata.is_dir() {
        let entries = fs::read_dir(path)?;
        let mut v: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                if let Ok(e) = entry.metadata() {
                    return e.is_file();
                }
                false
            })
            .map(|entry| entry.path())
            .collect();

        // read a file, if there are some files, sort and choose first.
        if v.len() > 0 {
            if v.len() > 1 {
                v.sort();
            }
            trace!("get file:{:?}", &v[0]);
            return fs::read(&v[0]);
        }
    }
    trace!("no file in [{:?}]", path);
    Ok(Vec::new())
}

///get the system time.
pub(crate) fn now_str() -> String {
    let now = OffsetDateTime::now_utc();
    if let Ok(i) = UtcOffset::current_local_offset() {
        now.to_offset(i);
    }
    let mut date_time = now.to_string();
    if date_time.len() > 19 {
        date_time.truncate(19);
    }
    date_time
}
