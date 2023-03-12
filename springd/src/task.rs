use crate::dispatcher::DurationDispatcher;
use crate::{
    dispatcher::{CountDispatcher, Dispatcher},
    Arg,
};
use bytes::Bytes;
use log::{debug, error};
use num_cpus;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    multipart, Body, Client, Request, RequestBuilder,
};
use std::{collections::HashMap, fs as sfs, sync::Arc, time::Duration};
use tokio::{self, fs as tfs, runtime, sync as tsync};
use tokio_util::codec::{BytesCodec, FramedRead};

pub struct Task {
    arg: Arg,
    client: Client,
    dispatcher_lock: Arc<tsync::RwLock<Box<dyn Dispatcher>>>,
}

fn create_count_dispatcher(
    total: u64,
    rate: &Option<u16>,
) -> Box<dyn Dispatcher> {
    let count_dispatcher = CountDispatcher::new(total, rate);
    Box::new(count_dispatcher)
}

fn create_duration_dispatcher(
    duration: Duration,
    rate: &Option<u16>,
) -> Box<dyn Dispatcher> {
    let duration_dispatcher = DurationDispatcher::new(duration, rate);
    Box::new(duration_dispatcher)
}

impl Task {
    pub fn new(arg: Arg) -> anyhow::Result<Self> {
        let client = build_client(&arg)?;
        let dispatcher = if arg.requests.is_some() {
            Arc::new(tsync::RwLock::new(create_count_dispatcher(
                arg.requests.unwrap(),
                &arg.rate,
            )))
        } else {
            Arc::new(tsync::RwLock::new(create_duration_dispatcher(
                arg.duration.unwrap(),
                &arg.rate,
            )))
        };

        Ok(Self {
            arg,
            client,
            dispatcher_lock: dispatcher,
        })
    }

    async fn worker(self: Arc<Self>) {
        loop {
            if !self.dispatcher_lock.read().await.try_apply_job().await {
                break;
            }

            let request = build_request(&self.arg, &self.client).await;
            if request.is_err() {
                panic!("unknown fatal error: {:?}", request.err());
            }

            let response = self.client.execute(request.unwrap()).await;
            self.dispatcher_lock.read().await.complete_job();
            if response.is_err() {
                error!("execute request failed: {:?}", response.err());
            } else {
                let response = response.unwrap();
                error!(
                    "execute request succeeded, status: {:?}, headers: {:?}",
                    response.status(),
                    response.headers()
                );
            }
        }
    }

    pub fn run(self: Arc<Self>) -> anyhow::Result<()> {
        let rt = runtime::Builder::new_multi_thread()
            .worker_threads(num_cpus::get())
            .thread_name("springd-tokio-runtime-worker")
            .unhandled_panic(runtime::UnhandledPanic::ShutdownRuntime)
            .enable_all()
            .build()?;

        rt.block_on(async {
            let mut workers = Vec::with_capacity(self.arg.connections as usize);
            for _ in 0..self.arg.connections {
                workers.push(tokio::spawn(self.clone().worker()));
            }
            for worker in workers {
                let result = worker.await;
                if result.is_err() {
                    error!(
                        "worker execute request failed: {:?}",
                        result.unwrap_err()
                    );
                }
            }
        });

        Ok(())
    }
}

fn build_client(arg: &Arg) -> anyhow::Result<Client> {
    let mut builder = Client::builder();

    // build headers
    let mut headers = HeaderMap::new();
    for header in &arg.headers {
        let parts = header.trim().split_once(':');
        if let Some(parts) = parts {
            headers.insert(
                HeaderName::from_bytes(parts.0.as_bytes())?,
                HeaderValue::from_str(parts.1)?,
            );
        }
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
            let cert = sfs::read(cert)?;
            let key = sfs::read(key)?;
            let pkcs8 = reqwest::Identity::from_pkcs8_pem(&cert, &key)?;
            builder = builder.identity(pkcs8);
        }
    }

    match builder.build() {
        Ok(client) => Ok(client),
        Err(e) => Err(Box::new(e).into()),
    }
}

async fn set_request_text_body(
    arg: &Arg,
    mut builder: RequestBuilder,
) -> anyhow::Result<RequestBuilder> {
    if let Some(text_body) = &arg.text_body {
        builder = builder
            .body(Bytes::from(text_body.clone()))
            .header("Content-Type", "text/plain; charset=UTF-8");
    }

    if let Some(text_file) = &arg.text_file {
        let file = tfs::File::open(text_file).await?;
        builder = builder
            .body(file)
            .header("Content-Type", "text/plain; charset=UTF-8");
    }

    Ok(builder)
}

async fn set_request_json_body(
    arg: &Arg,
    mut builder: RequestBuilder,
) -> anyhow::Result<RequestBuilder> {
    if let Some(json_body) = &arg.json_body {
        builder = builder
            .body(Bytes::from(json_body.clone()))
            .header("Content-Type", "application/json; charset=UTF-8");
    }

    if let Some(json_file) = &arg.json_file {
        let file = tfs::File::open(json_file).await?;
        builder = builder
            .body(file)
            .header("Content-Type", "application/json; charset=UTF-8");
    }

    Ok(builder)
}

async fn set_request_form_body(
    arg: &Arg,
    mut builder: RequestBuilder,
) -> anyhow::Result<RequestBuilder> {
    if !arg.form.is_empty() {
        let mut params = HashMap::new();
        for kv in &arg.form {
            let parts = kv.trim().split_once(':');
            if let Some(v) = parts {
                params.insert(v.0, v.1);
            }
        }
        builder = builder.form(&params);
    }

    Ok(builder)
}

async fn set_request_multipart_body(
    arg: &Arg,
    mut builder: RequestBuilder,
) -> anyhow::Result<RequestBuilder> {
    if !arg.mp.is_empty() || !arg.mp_file.is_empty() {
        let mut form = multipart::Form::new();
        for kv in &arg.mp {
            let parts = kv.trim().split_once(':');
            if let Some(parts) = parts {
                let k = parts.0.to_string().clone().to_owned();
                let v = parts.0.to_string().clone().to_owned();
                form = form.text(k, v);
            }
        }

        // for uploading file
        for f in &arg.mp_file {
            let f = f.clone().to_owned();
            let name = f.file_name().unwrap().to_str().unwrap().to_owned();
            let name_copy = name.clone();

            let file = tfs::File::open(&f).await?;
            let stream = FramedRead::new(file, BytesCodec::new());
            let file_body = Body::wrap_stream(stream);

            // get file mime information
            let mime = mime_guess::from_path(&f);
            let mime = mime.first_or_octet_stream();
            let mime = mime.type_();
            let mime = mime.as_ref();

            form = form.part(
                name,
                multipart::Part::stream(file_body)
                    .file_name(name_copy)
                    .mime_str(mime)?,
            );
        }
        builder = builder.multipart(form);
    }

    Ok(builder)
}

async fn build_request(arg: &Arg, client: &Client) -> anyhow::Result<Request> {
    let mut builder = client.request(
        arg.method.to_reqwest_method(),
        arg.url.as_ref().unwrap().clone(),
    );

    // the following four types are mutually exclusive
    // only one will take effect
    builder = set_request_text_body(arg, builder).await?;
    builder = set_request_form_body(arg, builder).await?;
    builder = set_request_json_body(arg, builder).await?;
    builder = set_request_multipart_body(arg, builder).await?;

    match builder.build() {
        Ok(client) => Ok(client),
        Err(e) => Err(Box::new(e).into()),
    }
}
