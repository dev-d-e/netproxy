use super::*;
use httpenergy::*;

pub(crate) struct HttpRequest<'a> {
    req: H1RequestUnits,
    buf: SliceGet<'a>,
}

impl<'a> HttpRequest<'a> {
    ///parse request header
    pub(crate) fn parse(buf: &'a [u8]) -> Result<Self, &'static str> {
        trace!("request buf: {:?}", buf.len());
        let mut buf = buf.into();
        let req = H1RequestUnits::new(&mut buf);
        if req.err() {
            Err("method|target|version err")
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
        self.req
            .header_value_string(str, &mut self.buf)
            .unwrap_or_default()
    }

    pub(crate) fn get_host(&mut self) -> String {
        self.get_header_value("Host")
    }
}
