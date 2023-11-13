use log::trace;
use std::fs;
use std::path::PathBuf;
use tokio_native_tls::native_tls::Identity;

const CERTIFICATE: &str = "./cert";

fn get_certificate(path: &str) -> std::io::Result<Vec<u8>> {
    let metadata = fs::metadata(path)?;
    if metadata.is_file() {
        trace!("certificate file:{:?}", path);
        return fs::read(path);
    } else if metadata.is_dir() {
        let entries = fs::read_dir(path)?;
        let mut v: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                if let Ok(e) = entry.metadata() {
                    return e.is_file();
                }
                false
            })
            .map(|entry| entry.path())
            .collect();

        // read a file, if there are some files, sort and choose first.
        if v.len() > 0 {
            if v.len() > 1 {
                v.sort();
            }
            trace!("certificate file:{:?}", &v[0]);
            return fs::read(&v[0]);
        }
    }
    trace!("no certificate in [{:?}]", path);
    Ok(Vec::new())
}

pub(crate) fn get_identity() -> Option<Identity> {
    match get_certificate(CERTIFICATE) {
        Ok(_) => {}
        Err(_) => {}
    };
    None
}
