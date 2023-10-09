pub(crate) mod http;
pub(crate) mod tcp;
pub(crate) mod tls;
pub(crate) mod udp;

#[derive(Debug)]
struct Transfer(String, Vec<String>, Vec<u32>);
