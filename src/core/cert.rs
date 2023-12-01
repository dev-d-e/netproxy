use super::get_file;
use lazy_static::lazy_static;
use std::io::{Error, ErrorKind, Result};
use tokio::sync::Mutex;
use tokio_native_tls::native_tls::Identity;

lazy_static! {
    static ref VALID_IDENTITY: Mutex<Vec<Identity>> = Mutex::new(Vec::new());
}

async fn add_identity(identity: Identity) {
    VALID_IDENTITY.lock().await.push(identity);
}

async fn get_identity() -> Option<Identity> {
    VALID_IDENTITY.lock().await.first().cloned()
}

async fn build_identity(data: &[u8], pwd: &str) -> Option<Error> {
    match Identity::from_pkcs12(data, pwd) {
        Ok(identity) => {
            add_identity(identity).await;
            None
        }
        Err(e) => Some(Error::new(ErrorKind::InvalidData, e)),
    }
}

///get certificate info from "VALID_IDENTITY".
pub(crate) async fn valid_identity() -> Option<Identity> {
    get_identity().await
}

///input file path and  password
///get certificate data from file
///then build identity
pub(crate) async fn build_certificate_from_file(path: String, pwd: String) -> Result<usize> {
    let f = get_file(&path)?;
    if let Some(e) = build_identity(f.as_slice(), &pwd).await {
        return Err(e);
    }
    Ok(f.len())
}

///input socket and  password
///get certificate data from socket
///then build identity
pub(crate) async fn build_certificate_from_socket(_soc: String, _pwd: String) -> Result<usize> {
    Ok(0)
}
