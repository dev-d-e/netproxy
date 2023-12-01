use super::into_str;
use httparse::Request;
use httparse::Status::Complete;
use log::trace;
use std::collections::HashMap;
use std::io::{Error, ErrorKind::InvalidData, Result};

#[derive(Debug)]
pub(crate) struct HttpRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) _version: u8,
    pub(crate) headers: HashMap<String, Vec<u8>>,
    //byte offset to start of HTTP body
    pub(crate) _offset: u32,
}

impl HttpRequest {
    pub(crate) fn find_header_value(&self, str: &str) -> Option<String> {
        let v = self.headers.get(str)?;
        let mut v = v.to_vec();
        Some(into_str(&mut v))
    }

    pub(crate) fn find_host_value(&self) -> Option<String> {
        self.find_header_value("Host")
    }
}

///parse request header
pub(crate) fn parse_request(buf: &[u8]) -> Result<HttpRequest> {
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut req = Request::new(&mut headers);
    let mut offset = 0;
    if let Complete(n) = req.parse(buf).map_err(|e| Error::new(InvalidData, e))? {
        offset = n as u32;
    }

    let method = req.method.unwrap_or("").to_string();
    let path = req.path.unwrap_or("").to_string();
    let version = req.version.unwrap_or(0);
    let mut headers = HashMap::new();
    for h in req.headers {
        headers.insert(h.name.to_string(), h.value.to_vec());
    }
    trace!("parse_request header:{:?}", headers.keys());
    Ok(HttpRequest {
        method,
        path,
        _version: version,
        headers,
        _offset: offset,
    })
}
