use std::pin::Pin;
use std::time::{Duration, Instant};

/// A boxed, sendable future, as returned by [`Transport`] and [`Clock`].
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// One HTTP request handed to a [`Transport`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Request {
    /// `GET` or `POST`.
    pub method: &'static str,
    /// Absolute URL.
    pub url: String,
    /// `Accept`, `User-Agent`, and `Content-Type` with a body.
    pub headers: Vec<(&'static str, String)>,
    /// UTF-8 JSON body.
    pub body: Option<Vec<u8>>,
    /// How long the request may take in total.
    pub timeout: Duration,
}

/// What a [`Transport`] returns for any HTTP status.
#[derive(Debug, Clone)]
pub struct Response {
    /// HTTP status.
    pub status: u16,
    /// Raw body.
    pub body: Vec<u8>,
}

impl Response {
    /// Creates a response.
    pub fn new(status: u16, body: Vec<u8>) -> Self {
        Response { status, body }
    }
}

/// Why no response arrived.
#[derive(Debug)]
pub enum TransportFailure {
    /// [`Request::timeout`] passed.
    TimedOut,
    /// The connection failed or closed without a response.
    Failed(Box<dyn std::error::Error + Send + Sync>),
}

/// Sends one request. Return a [`Response`] for any status and a [`TransportFailure`] only when no
/// response arrived. It must not follow redirects.
pub trait Transport: Send + Sync {
    /// Sends `request`.
    fn send(&self, request: Request) -> BoxFuture<'_, Result<Response, TransportFailure>>;
}

/// Lets tests drive time without waiting.
pub trait Clock: Send + Sync {
    /// The current time.
    fn now(&self) -> Instant;
    /// Waits for `duration`.
    fn sleep(&self, duration: Duration) -> BoxFuture<'_, ()>;
}

pub(crate) struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, duration: Duration) -> BoxFuture<'_, ()> {
        Box::pin(tokio::time::sleep(duration))
    }
}

/// The default transport: a `reqwest::Client` that never follows redirects, so a request body
/// carrying the API key is not replayed to another location.
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    /// Builds the transport from `builder` (configure proxies or TLS on it); redirects are switched
    /// off.
    pub fn from_builder(builder: reqwest::ClientBuilder) -> Result<Self, reqwest::Error> {
        Ok(ReqwestTransport {
            client: builder
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        })
    }
}

impl Transport for ReqwestTransport {
    fn send(&self, request: Request) -> BoxFuture<'_, Result<Response, TransportFailure>> {
        Box::pin(async move {
            let method = if request.method == "POST" {
                reqwest::Method::POST
            } else {
                reqwest::Method::GET
            };
            // reqwest's timeout covers connecting, the headers and the whole body.
            let mut builder = self
                .client
                .request(method, &request.url)
                .timeout(request.timeout);
            for (name, value) in &request.headers {
                builder = builder.header(*name, value);
            }
            if let Some(body) = request.body {
                builder = builder.body(body);
            }
            let classify = |error: reqwest::Error| {
                if error.is_timeout() {
                    TransportFailure::TimedOut
                } else {
                    TransportFailure::Failed(Box::new(error))
                }
            };
            let response = builder.send().await.map_err(classify)?;
            let status = response.status().as_u16();
            let body = response.bytes().await.map_err(classify)?;
            Ok(Response {
                status,
                body: body.to_vec(),
            })
        })
    }
}
