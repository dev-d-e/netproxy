use httpenergy::H1RequestUnits;
use log::trace;

#[derive(Debug)]
pub(crate) struct HttpRequest<'a> {
    req: H1RequestUnits,
    buf: &'a [u8],
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
        self.req.header_value_string(str)
    }

    pub(crate) fn get_host(&mut self) -> String {
        self.get_header_value("Host")
    }
}

///parse request header
pub(crate) fn parse_request<'a>(buf: &'a [u8]) -> Result<HttpRequest, String> {
    trace!("request buf: {:?}", buf.len());
    let req = H1RequestUnits::new(buf);
    if req.is_err() {
        Err("request err".to_string())
    } else {
        Ok(HttpRequest { req, buf })
    }
}
