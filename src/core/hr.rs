use super::into_str;
use httpenergy::H1RequestUnits;
use log::trace;

pub(crate) struct HttpRequest<'a> {
    req: H1RequestUnits,
    buf: &'a [u8],
}

impl<'a> HttpRequest<'a> {
    ///parse request header
    pub(crate) fn parse(buf: &'a [u8]) -> Result<Self, String> {
        trace!("request buf: {:?}", buf.len());
        let req = H1RequestUnits::new(buf);
        if req.is_err() {
            Err("request err".to_string())
        } else {
            Ok(Self { req, buf })
        }
    }

    pub(crate) fn method(&mut self) -> String {
        into_str(self.req.method())
    }

    pub(crate) fn path(&mut self) -> String {
        into_str(self.req.target())
    }

    pub(crate) fn version(&mut self) -> String {
        into_str(self.req.version())
    }

    pub(crate) fn get_header_value(&mut self, str: &str) -> String {
        self.req.header_value_string(str)
    }

    pub(crate) fn get_host(&mut self) -> String {
        self.get_header_value("Host")
    }
}
