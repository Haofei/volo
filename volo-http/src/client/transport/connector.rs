use http::uri::Scheme;
use motore::service::UnaryService;
use volo::net::{Address, conn::Conn};

use super::{plain::PlainMakeConnection, protocol::ClientTransportConfig};
use crate::error::{ClientError, client::bad_scheme};

pub struct ConnectorBuilder<'a> {
    mk_conn: HttpMakeConnection,
    #[allow(unused)] // for non-tls
    config: &'a ClientTransportConfig,
}

impl<'a> ConnectorBuilder<'a> {
    pub fn new(config: &'a ClientTransportConfig) -> Self {
        let mk_conn = HttpMakeConnection::Plain(PlainMakeConnection::default());
        Self { mk_conn, config }
    }

    #[cfg(feature = "__tls")]
    pub fn with_tls(self) -> Self {
        self.with_tls_connector(default_tls_connector())
    }

    #[cfg(feature = "__tls")]
    pub fn with_tls_connector(self, tls_connector: volo::net::tls::TlsConnector) -> Self {
        let Self { mk_conn, config } = self;
        if config.disable_tls {
            panic!("Try calling `ConnectorBuilder::with_tls_connector` with TLS disabled");
        }
        let mk_conn = match mk_conn {
            HttpMakeConnection::Plain(plain) => {
                HttpMakeConnection::Tls(super::tls::TlsMakeConnection::new(plain, tls_connector))
            }
            HttpMakeConnection::Tls(tls) => HttpMakeConnection::Tls(tls),
        };

        Self { mk_conn, config }
    }

    pub fn build(self) -> HttpMakeConnection {
        let this = self;

        #[cfg(feature = "__tls")]
        let this = if this.config.disable_tls {
            this
        } else {
            // If the feature `tls` is enabled and it is not disabled by config, just use a default
            // config for creating a `TlsConnector`.
            this.with_tls()
        };

        this.mk_conn
    }
}

#[cfg(feature = "__tls")]
fn default_tls_connector() -> volo::net::tls::TlsConnector {
    volo::net::tls::TlsConnector::builder()
        .with_alpn_protocols([
            #[cfg(feature = "http2")]
            "h2",
            #[cfg(feature = "http1")]
            "http/1.1",
        ])
        .build()
        .unwrap_or_default()
}

#[derive(Clone, Debug)]
pub(super) struct PeerInfo {
    pub scheme: Scheme,
    pub address: Address,
    #[cfg(feature = "__tls")]
    pub name: faststr::FastStr,
}

#[derive(Clone, Debug)]
pub enum HttpMakeConnection {
    Plain(PlainMakeConnection),
    #[cfg(feature = "__tls")]
    Tls(super::tls::TlsMakeConnection),
}

impl HttpMakeConnection {
    pub fn builder(config: &ClientTransportConfig) -> ConnectorBuilder<'_> {
        ConnectorBuilder::new(config)
    }
}

impl UnaryService<PeerInfo> for HttpMakeConnection {
    type Response = Conn;
    type Error = ClientError;

    async fn call(&self, req: PeerInfo) -> Result<Self::Response, Self::Error> {
        match self {
            Self::Plain(plain) => {
                if req.scheme != Scheme::HTTP {
                    return Err(bad_scheme(req.scheme));
                }
                plain.call(req).await
            }
            #[cfg(feature = "__tls")]
            Self::Tls(tls) => {
                // FIXME: tokio-rustls does not support setting alpn for each connection
                tls.call(req).await
            }
        }
    }
}
