use super::into_str;
use httpenergy::{to_request, Request};
use log::trace;
use std::io::Result;

#[derive(Debug)]
pub(crate) struct HttpRequest {
    req: Request,
}

impl HttpRequest {
    pub(crate) fn method(&mut self) -> String {
        self.req.method().to_string()
    }

    pub(crate) fn path(&mut self) -> String {
        self.req.target().to_string()
    }

    pub(crate) fn version(&mut self) -> String {
        self.req.version().to_string()
    }

    pub(crate) fn find_header_value(&mut self, str: &str) -> Option<String> {
        let v = self.req.field(str)?;
        let mut v = v.to_vec();
        Some(into_str(&mut v))
    }

    pub(crate) fn find_host_value(&self) -> Option<String> {
        self.find_header_value("Host")
    }
}

///parse request header
pub(crate) fn parse_request(buf: &[u8]) -> Result<HttpRequest> {
    let req = to_request(buf);
    trace!("parse_request: {:?}", req);
    Ok(HttpRequest { req })
}
