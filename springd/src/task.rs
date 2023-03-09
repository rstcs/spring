use crate::task::dispatcher::{CountDispatcher, Dispatcher};
use crate::Arg;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};
use std::fs;

pub mod dispatcher;
pub mod limiter;

pub struct Task<'a> {
    arg: &'a Arg,
    client: Client,
    dispatcher: Box<dyn Dispatcher>,
}

impl<'a> Task<'a> {
    pub fn new(arg: &'a Arg) -> crate::error::Result<Self> {
        Ok(Self {
            arg,
            client: Self::build_client(arg)?,
            dispatcher: Box::new(CountDispatcher::new()),
        })
    }

    fn build_client(arg: &'a Arg) -> crate::error::Result<Client> {
        let mut builder = Client::builder();

        // build headers
        let mut headers = HeaderMap::new();
        for header in &arg.headers {
            let parts: Vec<_> = header.trim().split(':').collect();
            let mut value: HeaderValue = HeaderValue::from_static("");
            if parts.len() == 2 {
                value = HeaderValue::from_str(parts[1])?;
            }
            headers.insert(parts[0], value);
        }

        // disable http keep alive
        if arg.disable_keep_alive {
            headers.insert("Connection", HeaderValue::from_static("Close"));
        }

        builder = builder
            .default_headers(headers)
            .timeout(arg.timeout)
            .connect_timeout(arg.timeout)
            .danger_accept_invalid_certs(arg.insecure)
            .danger_accept_invalid_hostnames(arg.insecure);

        // use client certificates
        if let Some(cert) = &arg.cert {
            if let Some(key) = &arg.key {
                let cert = fs::read(cert)?;
                let key = fs::read(key)?;
                let pkcs8 = reqwest::Identity::from_pkcs8_pem(&cert, &key)?;
                builder = builder.identity(pkcs8);
            }
        }

        match builder.build() {
            Ok(client) => Ok(client),
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn worker(&self) {}
}
