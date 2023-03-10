use crate::task::dispatcher::{CountDispatcher, Dispatcher};
use crate::Arg;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client, Request,
};
use std::fs;

pub mod dispatcher;
pub mod limiter;

pub struct Task {
    arg: Arg,
    client: Client,
    dispatcher: Box<dyn Dispatcher>,
}

impl Task {
    pub fn new(arg: Arg, client: Client) -> anyhow::Result<Self> {
        Ok(Self {
            arg,
            client,
            dispatcher: Box::new(CountDispatcher::new()),
        })
    }

    async fn worker(&self) {}
}

fn build_client(arg: &Arg) -> anyhow::Result<Client> {
    let mut builder = Client::builder();

    // build headers
    let mut headers = HeaderMap::new();
    for header in &arg.headers {
        let parts: Vec<_> = header.trim().split(':').collect();
        let mut value: HeaderValue = HeaderValue::from_static("");
        if parts.len() == 2 {
            value = HeaderValue::from_str(parts[1])?;
        }
        headers.insert(HeaderName::from_bytes(parts[0].as_bytes())?, value);
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
        Err(e) => Err(Box::new(e).into()),
    }
}

fn build_request(arg: &Arg, client: &Client) -> anyhow::Result<Request> {
    let mut builder = client.request(
        arg.method.to_reqwest_method(),
        arg.url.as_ref().unwrap().clone(),
    );
    match builder.build() {
        Ok(client) => Ok(client),
        Err(e) => Err(Box::new(e).into()),
    }
}
