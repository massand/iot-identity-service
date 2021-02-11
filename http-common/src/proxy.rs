// Copyright (c) Microsoft. All rights reserved.

use openssl::pkey::{PKey, Private};

use std::io;

pub enum MaybeProxyConnector {
    NoProxy(hyper_openssl::HttpsConnector<hyper::client::HttpConnector>),
    Proxy(
        hyper_proxy::ProxyConnector<
            hyper_openssl::HttpsConnector<hyper::client::HttpConnector>,
        >,
    ),
}

impl MaybeProxyConnector {
    pub fn build(
        proxy_uri: Option<hyper::Uri>,
        identity: Option<(PKey<Private>, Vec<u8>)>,
    ) -> io::Result<Self> {
        let mut http_connector = hyper::client::HttpConnector::new();
        
        if let Some(proxy_uri) = proxy_uri {
            let proxy = uri_to_proxy(proxy_uri)?;
            let proxy_connector = match identity {
                None => {
                    let https_connector = hyper_openssl::HttpsConnector::new()?;
                    hyper_proxy::ProxyConnector::from_proxy(https_connector, proxy)?
                }
                Some((key, certs)) => { 
                    // DEVNOTE: SslConnectionBuilder::build() consumes the builder. So, we need
                    //          to create two copies of it.
                    let mut tls_connector =
                        openssl::ssl::SslConnector::builder(openssl::ssl::SslMethod::tls())?;
                    let mut proxy_tls_connector =
                        openssl::ssl::SslConnector::builder(openssl::ssl::SslMethod::tls())?;
                    let connectors = vec![tls_connector, proxy_tls_connector];
                    
                    let device_id_certs =
                        openssl::x509::X509::stack_from_pem(&certs)?.into_iter();
                    let client_cert = device_id_certs.next().ok_or_else(|| {
                        io::Error::new(io::ErrorKind::Other, "device identity cert not found")
                    })?;

                    for connector in connectors {
                        connector.set_certificate(&client_cert)?;
                
                        for cert in device_id_certs {
                            connector.add_extra_chain_cert(cert.clone())?;
                        }
                        
                        connector.set_private_key(&key);
                    }

                    let mut http_connector = hyper::client::HttpConnector::new();
                    http_connector.enforce_http(false);
                    let tls_connector =
                        hyper_openssl::HttpsConnector::with_connector(http_connector, tls_connector)?;
                    let proxy_connector = hyper_proxy::ProxyConnector::from_proxy(tls_connector, proxy)?;
                    proxy_connector.set_tls(Some(proxy_tls_connector.build()));
                    proxy_connector
                },
            };
            Ok(MaybeProxyConnector::Proxy(proxy_connector))
        } else {
            let https_connector = hyper_openssl::HttpsConnector::new()?;
            Ok(MaybeProxyConnector::NoProxy(https_connector))
        }
    }

    // pub fn request(&self, req: hyper::Request<hyper::Body>) -> hyper::client::ResponseFuture {
    //     match *self {
    //         MaybeProxyConnector::NoProxy(ref client) => client.request(req),
    //         MaybeProxyConnector::Proxy(ref client) => client.request(req),
    //     }
    // }
}

pub fn uri_to_proxy(uri: hyper::Uri) -> io::Result<hyper_proxy::Proxy> {
    let proxy_url =
        url::Url::parse(&uri.to_string()).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let mut proxy = hyper_proxy::Proxy::new(hyper_proxy::Intercept::All, uri);

    if !proxy_url.username().is_empty() {
        let username = percent_encoding::percent_decode_str(proxy_url.username())
            .decode_utf8()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let credentials = match proxy_url.password() {
            Some(password) => {
                let password = percent_encoding::percent_decode_str(password)
                    .decode_utf8()
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

                typed_headers::Credentials::basic(&username, &password)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            }
            None => typed_headers::Credentials::basic(&username, "")
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
        };
        proxy.set_authorization(credentials);
    }

    Ok(proxy)
}