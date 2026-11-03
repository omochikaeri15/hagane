use reqwest::blocking::{Client, ClientBuilder};

pub(crate) fn builder() -> ClientBuilder {
    trusted(Client::builder())
}

#[cfg(target_os = "android")]
fn trusted(builder: ClientBuilder) -> ClientBuilder {
    let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
    let config = rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();
    builder.tls_backend_preconfigured(config)
}

#[cfg(not(target_os = "android"))]
fn trusted(builder: ClientBuilder) -> ClientBuilder {
    builder
}
