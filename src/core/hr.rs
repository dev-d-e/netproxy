use httparse::Request;
use log::trace;
use std::collections::HashMap;
use std::io::ErrorKind;

#[derive(Debug)]
pub(crate) struct HttpRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) _version: u8,
    pub(crate) headers: HashMap<String, Vec<u8>>,
    //byte offset to start of HTTP body
    pub(crate) _offset: usize,
}

impl HttpRequest {
    pub(crate) fn find_header_value(&self, str: &str) -> Option<String> {
        let v = self.headers.get(str)?;
        match String::from_utf8(v.to_vec()) {
            Ok(s) => Some(s),
            Err(_) => None,
        }
    }

    pub(crate) fn find_host_value(&self) -> Option<String> {
        self.find_header_value("Host")
    }
}

pub(crate) fn parse_request(str: &[u8]) -> std::io::Result<HttpRequest> {
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut req = Request::new(&mut headers);
    let mut offset: usize = 0;
    let status = req
        .parse(str)
        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))?;

    if status.is_complete() {
        offset = status.unwrap();
    }

    trace!("httparse header:{:?}", req.headers);
    let method = req.method.unwrap_or("").to_string();
    let path = req.path.unwrap_or("").to_string();
    let version = req.version.unwrap_or(0);
    let mut headers = HashMap::new();
    for h in req.headers {
        headers.insert(h.name.to_string(), h.value.to_vec());
    }
    Ok(HttpRequest {
        method,
        path,
        _version: version,
        headers,
        _offset: offset,
    })
}
