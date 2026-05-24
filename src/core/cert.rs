use super::*;
use tokio::sync::{OnceCell, RwLock};
use tokio_native_tls::native_tls::Identity;

static VALID_IDENTITY: OnceCell<RwLock<Vec<Identity>>> = OnceCell::const_new();

async fn identity_vec() -> &'static RwLock<Vec<Identity>> {
    VALID_IDENTITY
        .get_or_init(|| async { RwLock::new(Vec::new()) })
        .await
}

async fn add_identity(identity: Identity) {
    identity_vec().await.write().await.push(identity);
}

async fn get_identity() -> Option<Identity> {
    identity_vec().await.read().await.first().cloned()
}

async fn build_identity(data: &[u8], pwd: &str) -> bool {
    match Identity::from_pkcs12(data, pwd) {
        Ok(i) => {
            add_identity(i).await;
            true
        }
        Err(e) => {
            error!("invalid data: {e}");
            false
        }
    }
}

///get certificate info from "VALID_IDENTITY".
pub(crate) async fn valid_identity() -> Option<Identity> {
    get_identity().await
}

///input file path and  password
///get certificate data from file
///then build identity
pub(crate) async fn build_certificate_from_file(path: String, pwd: String) -> bool {
    if let Some(f) = get_file(&path) {
        build_identity(f.as_slice(), &pwd).await
    } else {
        false
    }
}

///input socket and  password
///get certificate data from socket
///then build identity
pub(crate) async fn build_certificate_from_socket(_soc: String, _pwd: String) -> bool {
    false
}
