use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::error::{ApiError, ApiErrorKind, Error, Operation, SolveTimeoutError, TransportError};
use crate::error_codes;
use crate::transport::{
    Clock, Request, ReqwestTransport, Response, SystemClock, Transport, TransportFailure,
};
use crate::types::{ServiceInfo, SolveResult, TaskResult, TaskTypeCatalog};

/// The production API origin.
pub const DEFAULT_BASE_URL: &str = "https://api.sursolver.com";
/// Default bound on each HTTP request.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Default wait between polls in `solve`.
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(3);
/// Default `solve` deadline.
pub const DEFAULT_SOLVE_TIMEOUT: Duration = Duration::from_secs(210);
/// Default number of resubmissions after `ERROR_NO_SLOT_AVAILABLE`.
pub const DEFAULT_MAX_SUBMIT_RETRIES: u32 = 1;
/// Upper bound of the random delay added to a submit retry.
pub const SUBMIT_RETRY_JITTER: Duration = Duration::from_secs(1);
/// This crate's version, sent in the `User-Agent` header.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const LOOPBACK_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];

type RandomSource = Arc<dyn Fn() -> f64 + Send + Sync>;

/// Client of the SurSolver captcha-solving API. Cheap to clone; clones share one connection pool.
///
/// The API key travels only in request bodies and never appears in errors or `Debug` output.
/// Cancel a call by dropping its future (for example with `tokio::time::timeout` or `select!`).
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    api_key: String,
    base_url: String,
    request_timeout: Duration,
    poll_interval: Duration,
    solve_timeout: Duration,
    max_submit_retries: u32,
    app_id: Option<String>,
    user_agent: String,
    transport: Arc<dyn Transport>,
    clock: Arc<dyn Clock>,
    random: RandomSource,
}

/// Options of a [`Client`]. Every option has a working default.
pub struct ClientBuilder {
    api_key: String,
    base_url: String,
    request_timeout: Duration,
    poll_interval: Duration,
    solve_timeout: Duration,
    max_submit_retries: u32,
    app_id: Option<String>,
    user_agent_suffix: Option<String>,
    transport: Option<Arc<dyn Transport>>,
    clock: Option<Arc<dyn Clock>>,
    random: Option<RandomSource>,
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl ClientBuilder {
    /// API origin; plain `http://` only for localhost.
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Bound on each HTTP request (default 30 s).
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Wait between polls in `solve` (default 3 s).
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// `solve` deadline (default 210 s).
    pub fn solve_timeout(mut self, timeout: Duration) -> Self {
        self.solve_timeout = timeout;
        self
    }

    /// How often `solve` resubmits after `ERROR_NO_SLOT_AVAILABLE` (default 1).
    pub fn max_submit_retries(mut self, retries: u32) -> Self {
        self.max_submit_retries = retries;
        self
    }

    /// Your referral app id, sent with every `create_task`.
    pub fn app_id(mut self, app_id: impl Into<String>) -> Self {
        self.app_id = Some(app_id.into());
        self
    }

    /// Text appended to the `User-Agent` header `sursolver-rust/<version>`.
    pub fn user_agent_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.user_agent_suffix = Some(suffix.into());
        self
    }

    /// Replaces the HTTP transport (proxies, custom TLS, tests). See [`ReqwestTransport::from_builder`].
    pub fn transport(mut self, transport: impl Transport + 'static) -> Self {
        self.transport = Some(Arc::new(transport));
        self
    }

    /// Replaces the real clock (tests).
    pub fn clock(mut self, clock: impl Clock + 'static) -> Self {
        self.clock = Some(Arc::new(clock));
        self
    }

    /// Replaces the jitter source; it must return values in [0, 1).
    pub fn random(mut self, random: impl Fn() -> f64 + Send + Sync + 'static) -> Self {
        self.random = Some(Arc::new(random));
        self
    }

    /// Builds the client; an unusable option is an [`Error::Configuration`] and nothing is sent.
    pub fn build(self) -> Result<Client, Error> {
        let config = |message: &str| Err(Error::Configuration(message.to_owned()));
        if self.api_key.trim().is_empty() {
            return config("api_key must be a non-empty string");
        }
        if self.api_key.trim() != self.api_key {
            return config("api_key has leading or trailing whitespace");
        }
        let base_url = normalize_base_url(&self.base_url)?;
        for (name, value) in [
            ("request_timeout", self.request_timeout),
            ("poll_interval", self.poll_interval),
            ("solve_timeout", self.solve_timeout),
        ] {
            if value.is_zero() {
                return Err(Error::Configuration(format!("{name} must be positive")));
            }
        }
        let mut user_agent = format!("sursolver-rust/{VERSION}");
        if let Some(suffix) = &self.user_agent_suffix {
            if suffix.is_empty() || suffix.bytes().any(|b| !(0x20..=0x7e).contains(&b)) {
                return config("user_agent_suffix must be non-empty printable ASCII");
            }
            user_agent.push(' ');
            user_agent.push_str(suffix);
        }
        let transport: Arc<dyn Transport> = match self.transport {
            Some(transport) => transport,
            None => Arc::new(
                ReqwestTransport::from_builder(reqwest::Client::builder()).map_err(|e| {
                    Error::Configuration(format!("cannot start the HTTP client: {e}"))
                })?,
            ),
        };
        Ok(Client {
            inner: Arc::new(Inner {
                api_key: self.api_key,
                base_url,
                request_timeout: self.request_timeout,
                poll_interval: self.poll_interval,
                solve_timeout: self.solve_timeout,
                max_submit_retries: self.max_submit_retries,
                app_id: self
                    .app_id
                    .map(|id| id.trim().to_owned())
                    .filter(|id| !id.is_empty()),
                user_agent,
                transport,
                clock: self.clock.unwrap_or_else(|| Arc::new(SystemClock)),
                random: self.random.unwrap_or_else(|| Arc::new(fastrand_f64)),
            }),
        })
    }
}

type TaskCreatedCallback<'a> = Box<dyn FnOnce(&str) + Send + 'a>;

/// Per-call options of [`Client::solve_with`].
#[derive(Default)]
pub struct SolveOptions<'a> {
    poll_interval: Option<Duration>,
    solve_timeout: Option<Duration>,
    on_task_created: Option<TaskCreatedCallback<'a>>,
}

impl fmt::Debug for SolveOptions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SolveOptions")
            .field("poll_interval", &self.poll_interval)
            .field("solve_timeout", &self.solve_timeout)
            .finish_non_exhaustive()
    }
}

impl<'a> SolveOptions<'a> {
    /// Overrides the client's poll interval for this call.
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = Some(interval);
        self
    }

    /// Overrides the client's deadline for this call.
    pub fn solve_timeout(mut self, timeout: Duration) -> Self {
        self.solve_timeout = Some(timeout);
        self
    }

    /// Called once with the task id as soon as the task is accepted (and charged), so the id
    /// survives a dropped future or a [`SolveTimeoutError`].
    pub fn on_task_created(mut self, callback: impl FnOnce(&str) + Send + 'a) -> Self {
        self.on_task_created = Some(Box::new(callback));
        self
    }
}

impl fmt::Debug for Client {
    /// Never reveals the API key.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.inner.base_url)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// A client with default options.
    pub fn new(api_key: impl Into<String>) -> Result<Client, Error> {
        Client::builder(api_key).build()
    }

    /// A builder for a client with custom options.
    pub fn builder(api_key: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_owned(),
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            poll_interval: DEFAULT_POLL_INTERVAL,
            solve_timeout: DEFAULT_SOLVE_TIMEOUT,
            max_submit_retries: DEFAULT_MAX_SUBMIT_RETRIES,
            app_id: None,
            user_agent_suffix: None,
            transport: None,
            clock: None,
            random: None,
        }
    }

    /// Submits a task and returns its id. The task is charged from this point. It is never retried:
    /// after an [`Error::Transport`] the task may exist (check [`Client::get_balance`] before
    /// submitting again).
    ///
    /// `task` is `type` plus that type's fields, sent verbatim. Browser types require `userAgent`.
    pub async fn create_task(&self, task: &Value) -> Result<String, Error> {
        let inner = &self.inner;
        let mut payload = json!({ "clientKey": inner.api_key, "task": task });
        if let Some(app_id) = &inner.app_id {
            payload["appId"] = Value::from(app_id.as_str());
        }
        let body = self
            .solver_call(Operation::CreateTask, "/createTask", &payload, None)
            .await?;
        match body.get("taskId") {
            Some(Value::String(task_id)) if !task_id.is_empty() => Ok(task_id.clone()),
            _ => Err(protocol(
                Operation::CreateTask,
                "errorId 0 without a taskId",
            )),
        }
    }

    /// Reads a task's state once. A terminal failure is an [`ApiError`] of kind
    /// [`ApiErrorKind::TaskFailed`].
    pub async fn get_task_result(&self, task_id: &str) -> Result<TaskResult, Error> {
        let payload = json!({ "clientKey": self.inner.api_key, "taskId": task_id });
        let mut body = self
            .solver_call(
                Operation::GetTaskResult,
                "/getTaskResult",
                &payload,
                Some(task_id),
            )
            .await?;
        match body.get("status") {
            Some(Value::String(status)) if status == "processing" => Ok(TaskResult::Processing),
            Some(Value::String(status)) if status == "ready" => match body.remove("solution") {
                Some(Value::Object(solution)) => Ok(TaskResult::Ready(solution)),
                _ => Err(protocol(
                    Operation::GetTaskResult,
                    "status ready without a solution object",
                )),
            },
            status => Err(protocol(
                Operation::GetTaskResult,
                &format!("unknown status {}", status.unwrap_or(&Value::Null)),
            )),
        }
    }

    /// The cash balance in USD as the JSON number the API sends. Display it; do not do money
    /// arithmetic on it (the dashboard's `GET /v1/balance` has the exact decimal).
    pub async fn get_balance(&self) -> Result<f64, Error> {
        let payload = json!({ "clientKey": self.inner.api_key });
        let body = self
            .solver_call(Operation::GetBalance, "/getBalance", &payload, None)
            .await?;
        match body.get("balance").and_then(Value::as_f64) {
            Some(balance) if balance.is_finite() => Ok(balance),
            _ => Err(protocol(
                Operation::GetBalance,
                "errorId 0 without a numeric balance",
            )),
        }
    }

    /// Creates a task and polls it until it is ready, failed, or the deadline passes, with the
    /// client's settings. See [`Client::solve_with`].
    pub async fn solve(&self, task: &Value) -> Result<SolveResult, Error> {
        self.solve_with(task, SolveOptions::default()).await
    }

    /// Creates a task and polls it until it is ready, failed, or the deadline passes.
    ///
    /// `createTask` is resubmitted only after `ERROR_NO_SLOT_AVAILABLE` (nothing was created), at
    /// most `max_submit_retries` times; any other submit failure is returned at once. Polls that fail
    /// with an [`Error::Transport`] or `ERROR_INTERNAL_ERROR` are retried until the deadline; any
    /// other error ends the call and carries the task id. On the deadline an [`Error::SolveTimeout`]
    /// is returned; the task may still finish.
    pub async fn solve_with(
        &self,
        task: &Value,
        options: SolveOptions<'_>,
    ) -> Result<SolveResult, Error> {
        let inner = &self.inner;
        let interval = options.poll_interval.unwrap_or(inner.poll_interval);
        let timeout = options.solve_timeout.unwrap_or(inner.solve_timeout);
        if interval.is_zero() || timeout.is_zero() {
            return Err(Error::Configuration(
                "poll_interval and solve_timeout must be positive".to_owned(),
            ));
        }
        let deadline = inner.clock.now() + timeout;

        let mut retries = 0;
        let task_id = loop {
            match self.create_task(task).await {
                Ok(task_id) => break task_id,
                Err(Error::Api(error))
                    if error.kind == ApiErrorKind::NoSlotAvailable
                        && retries < inner.max_submit_retries =>
                {
                    retries += 1;
                    let jitter =
                        ((inner.random)() * SUBMIT_RETRY_JITTER.as_millis() as f64).floor() as u64;
                    inner
                        .clock
                        .sleep(interval + Duration::from_millis(jitter))
                        .await;
                }
                Err(error) => return Err(error),
            }
        };
        if let Some(callback) = options.on_task_created {
            callback(&task_id);
        }

        loop {
            let mut last_error = None;
            match self.get_task_result(&task_id).await {
                Ok(TaskResult::Ready(solution)) => return Ok(SolveResult { task_id, solution }),
                Ok(TaskResult::Processing) => {}
                Err(error @ Error::Transport(_)) => last_error = Some(Box::new(error)),
                Err(Error::Api(error)) if error.code == error_codes::ERROR_INTERNAL_ERROR => {
                    last_error = Some(Box::new(Error::Api(error)));
                }
                Err(error) => return Err(error),
            }
            inner.clock.sleep(interval).await;
            if inner.clock.now() >= deadline {
                return Err(Error::SolveTimeout(SolveTimeoutError {
                    task_id,
                    timeout,
                    last_error,
                }));
            }
        }
    }

    /// The public task-type catalog: accepted names, fields, proxy modes and result keys.
    pub async fn list_task_types(&self) -> Result<TaskTypeCatalog, Error> {
        self.catalog_call(Operation::ListTaskTypes, "/v1/task-types")
            .await
    }

    /// Active services with their current USD price per task (decimal strings).
    pub async fn list_services(&self) -> Result<Vec<ServiceInfo>, Error> {
        #[derive(serde::Deserialize)]
        struct Data {
            services: Vec<ServiceInfo>,
        }
        let data: Data = self
            .catalog_call(Operation::ListServices, "/v1/services")
            .await?;
        Ok(data.services)
    }

    fn request(&self, method: &'static str, path: &str, body: Option<Vec<u8>>) -> Request {
        let mut headers = vec![
            ("Accept", "application/json".to_owned()),
            ("User-Agent", self.inner.user_agent.clone()),
        ];
        if body.is_some() {
            headers.push(("Content-Type", "application/json".to_owned()));
        }
        Request {
            method,
            url: format!("{}{}", self.inner.base_url, path),
            headers,
            body,
            timeout: self.inner.request_timeout,
        }
    }

    async fn send(&self, operation: Operation, request: Request) -> Result<Response, Error> {
        let timeout = request.timeout;
        let response = match self.inner.transport.send(request).await {
            Ok(response) => response,
            Err(failure) => {
                let (reason, source) = match failure {
                    TransportFailure::TimedOut => (
                        format!("request timed out after {} ms", timeout.as_millis()),
                        None,
                    ),
                    TransportFailure::Failed(source) => {
                        ("connection failed".to_owned(), Some(source))
                    }
                };
                return Err(transport_error(operation, reason, None, None, None, source));
            }
        };
        if response.status == 200 {
            return Ok(response);
        }
        let envelope = serde_json::from_slice::<Value>(&response.body).ok();
        let detail = envelope
            .as_ref()
            .and_then(|v| v.get("error"))
            .and_then(Value::as_object);
        let server_code = detail
            .and_then(|d| d.get("code"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let retry_after = detail
            .and_then(|d| d.get("retry_after_seconds"))
            .and_then(Value::as_f64)
            .map(|s| s as i64);
        let reason = match &server_code {
            Some(code) => format!("HTTP {} ({code})", response.status),
            None => format!("HTTP {}", response.status),
        };
        Err(transport_error(
            operation,
            reason,
            Some(response.status),
            server_code,
            retry_after,
            None,
        ))
    }

    /// POSTs to a solver endpoint; returns the errorId 0 body or the classified error.
    async fn solver_call(
        &self,
        operation: Operation,
        path: &str,
        payload: &Value,
        task_id: Option<&str>,
    ) -> Result<Map<String, Value>, Error> {
        let encoded = serde_json::to_vec(payload).expect("a JSON value always serializes");
        let response = self
            .send(operation, self.request("POST", path, Some(encoded)))
            .await?;
        let parsed = serde_json::from_slice::<Value>(&response.body);
        let not_envelope = |what: &str| {
            Err(transport_error(
                operation,
                format!("HTTP 200 body is {what}"),
                Some(200),
                None,
                None,
                None,
            ))
        };
        let mut body = match parsed {
            Err(_) => return not_envelope("not JSON"),
            Ok(Value::Object(body)) => body,
            Ok(_) => return not_envelope("not the solver envelope"),
        };
        let error_id = match body.get("errorId").and_then(Value::as_f64) {
            Some(id) if id.fract() == 0.0 => id,
            _ => return not_envelope("not the solver envelope"),
        };
        if error_id == 0.0 {
            return Ok(body);
        }
        if error_id != 1.0 {
            return Err(protocol(operation, &format!("errorId {}", body["errorId"])));
        }
        let code = match body.remove("errorCode") {
            Some(Value::String(code)) => code,
            _ => return Err(protocol(operation, "errorId 1 without a string errorCode")),
        };
        let description = match body.remove("errorDescription") {
            Some(Value::String(text)) => text,
            _ => String::new(),
        };
        let supported = match body.remove("supportedUserAgents") {
            Some(Value::Array(list)) => list
                .into_iter()
                .filter_map(|entry| match entry {
                    Value::String(ua) => Some(ua),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        Err(Error::Api(ApiError::new(
            operation,
            code,
            description,
            task_id.map(str::to_owned),
            supported,
        )))
    }

    /// GETs a public catalog endpoint and decodes its `data` object.
    async fn catalog_call<T: DeserializeOwned>(
        &self,
        operation: Operation,
        path: &str,
    ) -> Result<T, Error> {
        let response = self
            .send(operation, self.request("GET", path, None))
            .await?;
        let data = match serde_json::from_slice::<Value>(&response.body) {
            Ok(Value::Object(mut envelope))
                if envelope.get("success") == Some(&Value::Bool(true)) =>
            {
                match envelope.remove("data") {
                    Some(data @ Value::Object(_)) => data,
                    _ => None.ok_or_else(|| catalog_envelope_error(operation))?,
                }
            }
            _ => return Err(catalog_envelope_error(operation)),
        };
        serde_json::from_value(data).map_err(|e| {
            protocol(
                operation,
                &format!("a catalog field is missing or mistyped: {e}"),
            )
        })
    }
}

fn catalog_envelope_error(operation: Operation) -> Error {
    transport_error(
        operation,
        "HTTP 200 body is not the API envelope".to_owned(),
        Some(200),
        None,
        None,
        None,
    )
}

fn transport_error(
    operation: Operation,
    reason: String,
    status: Option<u16>,
    server_code: Option<String>,
    retry_after_seconds: Option<i64>,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
) -> Error {
    Error::Transport(TransportError {
        operation,
        reason,
        status,
        server_code,
        retry_after_seconds,
        source,
    })
}

fn protocol(operation: Operation, detail: &str) -> Error {
    Error::Protocol {
        operation,
        detail: detail.to_owned(),
    }
}

fn normalize_base_url(raw: &str) -> Result<String, Error> {
    let url = reqwest::Url::parse(raw)
        .ok()
        .filter(|url| url.has_host())
        .ok_or_else(|| Error::Configuration("base_url is not a valid absolute URL".to_owned()))?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let loopback_http = url.scheme() == "http" && LOOPBACK_HOSTS.contains(&host.as_str());
    if url.scheme() != "https" && !loopback_http {
        return Err(Error::Configuration(
            "base_url must use https:// (plain http:// only for localhost)".to_owned(),
        ));
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || raw.contains('?')
        || raw.contains('#')
    {
        return Err(Error::Configuration(
            "base_url must not carry credentials, a query or a fragment".to_owned(),
        ));
    }
    let port = url.port().map(|p| format!(":{p}")).unwrap_or_default();
    Ok(format!(
        "{}://{}{}{}",
        url.scheme(),
        host,
        port,
        url.path().trim_end_matches('/')
    ))
}

/// Jitter in [0, 1) from the standard library's randomly seeded hasher; no extra dependency.
fn fastrand_f64() -> f64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    (hasher.finish() >> 11) as f64 / (1u64 << 53) as f64
}
