use httpenergy::{new_request_units, RequestUnits};
use log::trace;
use std::io::Result;

#[derive(Debug)]
pub(crate) struct HttpRequest<'a> {
    req: RequestUnits,
    buf: &'a Vec<u8>,
}

impl<'a> HttpRequest<'a> {
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
        let buf = self.buf.as_slice();
        self.req.header_value_string(str, buf)
    }

    pub(crate) fn get_host(&mut self) -> String {
        self.get_header_value("Host")
    }
}

///parse request header
pub(crate) fn parse_request<'a>(buf: &'a Vec<u8>) -> Result<HttpRequest> {
    trace!("request buf: {:?}", buf.len());
    let req = new_request_units(buf);
    Ok(HttpRequest { req, buf })
}
