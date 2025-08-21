mod cert;
mod hr;
mod pd;
mod pdtrait;
mod rt;
mod rw;
mod sv;

pub(crate) use cert::*;
use getset::{CopyGetters, Getters};
pub(crate) use hr::*;
use log::trace;
pub(crate) use pd::*;
pub(crate) use pdtrait::*;
pub(crate) use rt::*;
pub(crate) use rw::*;
use std::fs;
use std::path::PathBuf;
pub(crate) use sv::*;
use time::{OffsetDateTime, UtcOffset};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Protoc {
    TCP,
    TLS,
    UDP,
    HTTP,
    HTTPPT,
}

#[derive(CopyGetters, Getters)]
pub(crate) struct Remote {
    #[getset(get_copy = "pub(crate)")]
    protoc: Protoc,
    #[getset(get = "pub(crate)")]
    target: String,
    #[getset(get = "pub(crate)")]
    host: String,
}

impl Remote {
    pub(crate) fn new(protoc: Protoc, target: String, host: String) -> Self {
        Self {
            protoc,
            target,
            host,
        }
    }
}

struct StrWrapper(String);

impl utf8parse::Receiver for StrWrapper {
    fn codepoint(&mut self, c: char) {
        self.0.push(c);
    }

    fn invalid_sequence(&mut self) {}
}

pub(crate) fn into_str(buf: &[u8]) -> String {
    trace!("into_str:{:?}", buf.len());
    let mut p = utf8parse::Parser::new();
    let mut t = StrWrapper(String::new());
    for byte in buf {
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
    let f = time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    now.format(f).unwrap_or_else(|_| now.to_string())
}

pub(crate) fn divide(v: &mut Vec<usize>) {
    let mut d = 1;
    let mut iter = v.windows(2);
    while let Some(o) = iter.next() {
        let i = co_divisor(o[0], o[1]);
        if i <= 1 {
            d = 1;
            break;
        }
        if i < d || d <= 1 {
            d = i;
        }
    }
    while d > 1 {
        if v.iter().any(|n| *n % d != 0) {
            d -= 1;
        } else {
            break;
        }
    }
    if d > 1 {
        v.iter_mut().for_each(|n| *n /= d)
    }
}

//calculate common divisor.
fn co_divisor(a: usize, b: usize) -> usize {
    let c = a % b;
    if c == 0 {
        b
    } else {
        co_divisor(b, c)
    }
}

#[test]
fn test() {
    println!("now: {:?}", now_str());
    let mut v = vec![2, 4, 6];
    divide(&mut v);
    assert_eq!(v, [1, 2, 3]);
}
