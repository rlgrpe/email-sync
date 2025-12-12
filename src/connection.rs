//! Internal module for establishing TLS connections to IMAP servers.
//!
//! Supports both direct connections and SOCKS5 proxy connections.

use crate::error::{Error, Result};
use crate::proxy::Socks5Proxy;
use rustls::ClientConfig;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_socks::tcp::Socks5Stream;
use tracing::{debug, instrument};
use webpki_roots::TLS_SERVER_ROOTS;

/// A TLS stream over TCP, used for IMAP communication.
pub(crate) type TlsStream = tokio_rustls::client::TlsStream<TcpStream>;

/// Establishes a TLS connection to an IMAP server.
///
/// If a proxy is provided, the connection is routed through SOCKS5.
#[instrument(
    name = "connection::establish_tls",
    skip_all,
    fields(
        imap_host = %imap_host,
        target_addr = %target_addr,
        proxy_enabled = proxy.is_some()
    )
)]
pub(crate) async fn establish_tls_connection(
    imap_host: &str,
    target_addr: &str,
    proxy: Option<&Socks5Proxy>,
) -> Result<TlsStream> {
    let connector = create_tls_connector();
    let server_name = parse_server_name(imap_host)?;
    let tcp_stream = connect_tcp(target_addr, proxy).await?;

    debug!("Performing TLS handshake");

    connector
        .connect(server_name, tcp_stream)
        .await
        .map_err(|source| Error::TlsConnect {
            target: target_addr.to_string(),
            source,
        })
}

/// Creates a TLS connector with system root certificates.
fn create_tls_connector() -> TlsConnector {
    let mut root_cert_store = rustls::RootCertStore::empty();
    root_cert_store.add_trust_anchors(TLS_SERVER_ROOTS.iter().map(|ta| {
        rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));

    let tls_config = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();

    TlsConnector::from(Arc::new(tls_config))
}

/// Parses server name for TLS SNI.
fn parse_server_name(host: &str) -> Result<rustls::ServerName> {
    rustls::ServerName::try_from(host).map_err(|source| Error::InvalidDnsName {
        host: host.to_string(),
        source,
    })
}

/// Establishes a TCP connection, optionally through SOCKS5.
#[instrument(
    name = "connection::tcp_connect",
    skip_all,
    fields(
        target_addr = %target_addr,
        via_proxy = proxy.is_some()
    )
)]
async fn connect_tcp(target_addr: &str, proxy: Option<&Socks5Proxy>) -> Result<TcpStream> {
    match proxy {
        Some(proxy) => connect_via_socks5(target_addr, proxy).await,
        None => connect_direct(target_addr).await,
    }
}

/// Direct TCP connection.
#[instrument(name = "connection::direct", skip_all)]
async fn connect_direct(target_addr: &str) -> Result<TcpStream> {
    debug!(target = %target_addr, "Establishing direct TCP connection");

    TcpStream::connect(target_addr)
        .await
        .map_err(|source| Error::TcpConnect {
            target: target_addr.to_string(),
            source,
        })
}

/// TCP connection via SOCKS5 proxy.
#[instrument(
    name = "connection::socks5",
    skip_all,
    fields(
        proxy_host = %proxy.host,
        has_auth = proxy.requires_auth()
    )
)]
async fn connect_via_socks5(target_addr: &str, proxy: &Socks5Proxy) -> Result<TcpStream> {
    debug!(
        proxy = %proxy,
        target = %target_addr,
        "Connecting via SOCKS5 proxy"
    );

    let proxy_addr = (proxy.host.as_str(), proxy.port);

    let stream = match &proxy.auth {
        Some(auth) => {
            Socks5Stream::connect_with_password(
                proxy_addr,
                target_addr,
                &auth.username,
                &auth.password,
            )
            .await
        }
        None => Socks5Stream::connect(proxy_addr, target_addr).await,
    };

    stream
        .map(Socks5Stream::into_inner)
        .map_err(|source| Error::Socks5Connect {
            proxy_host: proxy.host.clone(),
            target: target_addr.to_string(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_server_name() {
        let result = parse_server_name("imap.gmail.com");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_invalid_server_name() {
        // Empty string should fail
        let result = parse_server_name("");
        assert!(result.is_err());
    }
}
