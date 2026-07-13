//! Load rustls server certificates for in-process TLS (experimental MCP HTTP).

use rustls::pki_types::CertificateDer;
use rustls::ServerConfig;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

/// Load optional TLS server config from two environment variable names.
/// Both variables must be set together, or neither.
pub fn load_server_config_from_env(
    cert_var: &str,
    key_var: &str,
) -> Result<Option<Arc<ServerConfig>>, String> {
    let cert = std::env::var(cert_var).ok();
    let key = std::env::var(key_var).ok();
    match (cert, key) {
        (Some(cert_path), Some(key_path)) => Ok(Some(load_server_config(
            Path::new(&cert_path),
            Path::new(&key_path),
        )?)),
        (None, None) => Ok(None),
        _ => Err(format!(
            "TLS requires both {cert_var} and {key_var} (or neither)"
        )),
    }
}

/// Build a TLS server config from PEM certificate and private-key files.
pub fn load_server_config(cert_path: &Path, key_path: &Path) -> Result<Arc<ServerConfig>, String> {
    let cert_file = File::open(cert_path)
        .map_err(|e| format!("cannot open TLS cert {}: {e}", cert_path.display()))?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("invalid TLS cert PEM: {e}"))?;
    if certs.is_empty() {
        return Err("TLS cert file contained no certificates".into());
    }

    let key_file = File::open(key_path)
        .map_err(|e| format!("cannot open TLS key {}: {e}", key_path.display()))?;
    let mut key_reader = BufReader::new(key_file);
    let key = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|e| format!("invalid TLS key PEM: {e}"))?
        .ok_or_else(|| "TLS key file contained no private key".to_string())?;

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map(Arc::new)
        .map_err(|e| format!("TLS server config failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_cert_reports_clear_error() {
        let err = load_server_config(Path::new("/nonexistent/cert.pem"), Path::new("/x.key"))
            .unwrap_err();
        assert!(err.contains("cert"));
    }

    #[test]
    fn partial_tls_env_reports_clear_error() {
        let _cert =
            crate::test_support::EnvVarGuard::set("GHOST_TEST_TLS_CERT_ONLY", "/tmp/cert.pem");
        let _key = crate::test_support::EnvVarGuard::remove("GHOST_TEST_TLS_KEY_ONLY");
        let err =
            load_server_config_from_env("GHOST_TEST_TLS_CERT_ONLY", "GHOST_TEST_TLS_KEY_ONLY")
                .unwrap_err();
        assert!(err.contains("GHOST_TEST_TLS_CERT_ONLY"));
        assert!(err.contains("GHOST_TEST_TLS_KEY_ONLY"));
    }
}
