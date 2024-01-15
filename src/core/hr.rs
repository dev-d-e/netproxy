use httpenergy::{to_request, RequestUnits};
use log::trace;
use std::io::Result;

#[derive(Debug)]
pub(crate) struct HttpRequest {
    req: RequestUnits,
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

    pub(crate) fn get_header_value(&mut self, str: &str) -> String {
        self.req.header_value_string(str)
    }

    pub(crate) fn get_host(&mut self) -> String {
        self.get_header_value("Host")
    }
}

///parse request header
pub(crate) fn parse_request(buf: &[u8]) -> Result<HttpRequest> {
    trace!("request buf: {:?}", buf.len());
    let req = to_request(buf);
    Ok(HttpRequest { req })
}
