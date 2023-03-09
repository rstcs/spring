use crate::task::dispatcher::{CountDispatcher, Dispatcher};
use crate::Arg;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};

pub mod dispatcher;
pub mod limiter;

pub struct Task<'a> {
    arg: &'a Arg,
    client: Client,
    dispatcher: Box<dyn Dispatcher>,
}

impl<'a> Task<'a> {
    pub fn new(arg: &'a Arg) -> Self {
        Self {
            arg,
            dispatcher: Box::new(CountDispatcher::new()),
        }
    }

    fn build_client(arg: &'a Arg) -> crate::error::Result<Client> {
        let builder = Client::builder();

        // build headers
        let mut headers = HeaderMap::new();
        for header in &arg.headers {
            let mut parts: Vec<_> = header.trim().split(':').collect();
            if parts.len() == 0 {
                continue;
            }

            let value: HeaderValue;
            if parts.len() == 1 {
                value = HeaderValue::from_static("");
            } else {
                value = HeaderValue::from_str(parts[1])?;
            }
            headers.insert(parts[0], value);
        }

        Ok(Client {})
    }

    async fn worker(&self) {}
}
