//! Runs the vendored language-neutral conformance scenarios (conformance/README.md) with a
//! scripted fake transport, a virtual clock and a fixed random source.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use regex_lite::Regex;
use serde_json::{Map, Value, json};
use sursolver::{
    ApiErrorKind, BoxFuture, Client, Clock, Error, Request, Response, SolveOptions, Transport,
    TransportFailure,
};
use tokio::sync::Notify;

pub const TEST_API_KEY: &str = "sur_00000000000000000000000000000000";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance")
}

struct ScriptState {
    script: Vec<Value>,
    requests: Vec<Request>,
    failures: Vec<String>,
}

#[derive(Clone)]
struct ScriptedTransport(Arc<Mutex<ScriptState>>);

impl Transport for ScriptedTransport {
    fn send(&self, request: Request) -> BoxFuture<'_, Result<Response, TransportFailure>> {
        let outcome = self.respond(request);
        Box::pin(async move { outcome })
    }
}

impl ScriptedTransport {
    fn respond(&self, request: Request) -> Result<Response, TransportFailure> {
        let mut state = self.0.lock().unwrap();
        state.requests.push(request.clone());
        let step = state.requests.len();
        let entry = state.script.get(step - 1).cloned();
        let mut mismatch = |message: String| {
            state.failures.push(format!("request {step}: {message}"));
            Err(TransportFailure::Failed("harness mismatch".into()))
        };
        let Some(entry) = entry else {
            return mismatch(format!("unexpected {} {}", request.method, request.url));
        };
        let expected = &entry["expect"];
        let url = reqwest::Url::parse(&request.url).unwrap();
        if expected["method"] != request.method || expected["path"] != url.path() {
            return mismatch(format!(
                "expected {} {}, got {} {}",
                expected["method"],
                expected["path"],
                request.method,
                url.path()
            ));
        }
        if expected["noQuery"] == true && url.query().is_some() {
            return mismatch("unexpected query".into());
        }
        if let Some(want) = expected.get("json") {
            let body: Value = serde_json::from_slice(request.body.as_deref().unwrap_or_default())
                .unwrap_or(Value::Null);
            if !json_eq(want, &body) {
                return mismatch(format!("body {body} != {want}"));
            }
        }
        if let Some(Value::Object(headers)) = expected.get("headers") {
            for (name, want) in headers {
                let actual = request
                    .headers
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(name))
                    .map(|(_, v)| v.as_str());
                let matches = match (actual, want) {
                    (Some(actual), Value::Object(pattern)) => {
                        Regex::new(pattern["matches"].as_str().unwrap())
                            .unwrap()
                            .is_match(actual)
                    }
                    (Some(actual), Value::String(want)) => actual == want,
                    _ => false,
                };
                if !matches {
                    return mismatch(format!("header {name} mismatch: {actual:?}"));
                }
            }
        }
        match entry.get("fail").and_then(Value::as_str) {
            None => {}
            Some("request_timeout") => return Err(TransportFailure::TimedOut),
            Some("connection_reset") => {
                return Err(TransportFailure::Failed(Box::new(std::io::Error::from(
                    std::io::ErrorKind::ConnectionReset,
                ))));
            }
            Some(_) => {
                return Err(TransportFailure::Failed(Box::new(std::io::Error::from(
                    std::io::ErrorKind::UnexpectedEof,
                ))));
            }
        }
        Ok(scripted_response(&entry["respond"]))
    }
}

fn scripted_response(respond: &Value) -> Response {
    let mut status = respond.get("status").and_then(Value::as_u64).unwrap_or(200) as u16;
    if let Some(file) = respond.get("respondFrom").and_then(Value::as_str) {
        let fixture: Value = serde_json::from_str(
            &std::fs::read_to_string(root().join("recorded").join(file)).unwrap(),
        )
        .unwrap();
        let fixture = &fixture["response"];
        if respond.get("status").is_none() {
            status = fixture["status"].as_u64().unwrap() as u16;
        }
        let body = if fixture["body_is_json"] == true {
            fixture["body"].to_string()
        } else {
            fixture["body"].as_str().unwrap().to_owned()
        };
        return Response::new(status, body.into_bytes());
    }
    if let Some(payload) = respond.get("json") {
        return Response::new(status, payload.to_string().into_bytes());
    }
    Response::new(
        status,
        respond
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .as_bytes()
            .to_vec(),
    )
}

struct ClockState {
    elapsed: Duration,
    sleeps: Vec<u64>,
}

#[derive(Clone)]
struct VirtualClock {
    start: Instant,
    state: Arc<Mutex<ClockState>>,
    cancel_at: usize,
    cancelled: Arc<Notify>,
}

impl Clock for VirtualClock {
    fn now(&self) -> Instant {
        self.start + self.state.lock().unwrap().elapsed
    }

    fn sleep(&self, duration: Duration) -> BoxFuture<'_, ()> {
        let mut state = self.state.lock().unwrap();
        state.sleeps.push(duration.as_millis() as u64);
        state.elapsed += duration;
        if state.sleeps.len() == self.cancel_at {
            // The caller drops the solve future: Rust's cancellation.
            self.cancelled.notify_one();
            return Box::pin(std::future::pending());
        }
        Box::pin(async {})
    }
}

/// The README key rule: lowercase without underscores, except inside a solution.
fn normalize(value: &Value, inside_solution: bool) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|v| normalize(v, inside_solution))
                .collect(),
        ),
        Value::Object(map) if !inside_solution => Value::Object(
            map.iter()
                .map(|(k, v)| {
                    (
                        k.replace('_', "").to_lowercase(),
                        normalize(v, k == "solution"),
                    )
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| json_eq(a, b))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| json_eq(v, w)))
        }
        _ => a == b,
    }
}

fn pointer_get<'a>(root: &'a Value, pointer: &str) -> &'a Value {
    root.pointer(pointer).unwrap_or(&Value::Null)
}

/// An error's spec type name and fields under the scenario's names.
fn error_view(error: &Error) -> (String, Map<String, Value>) {
    let mut fields = Map::new();
    if let Some(operation) = error.operation() {
        fields.insert("operation".into(), json!(operation.as_str()));
    }
    let kind = match error {
        Error::Configuration(_) => "ConfigurationError",
        Error::Protocol { .. } => "ProtocolError",
        Error::SolveTimeout(timeout) => {
            fields.insert("taskId".into(), json!(timeout.task_id));
            fields.insert(
                "lastErrorType".into(),
                json!(timeout.last_error.as_ref().map(|e| error_view(e).0)),
            );
            "SolveTimeoutError"
        }
        Error::Transport(transport) => {
            fields.insert(
                "mayHaveCreatedTask".into(),
                json!(transport.may_have_created_task()),
            );
            if let Some(status) = transport.status {
                fields.insert("status".into(), json!(status));
            }
            if let Some(code) = &transport.server_code {
                fields.insert("serverCode".into(), json!(code));
            }
            if let Some(seconds) = transport.retry_after_seconds {
                fields.insert("retryAfterSeconds".into(), json!(seconds));
            }
            "TransportError"
        }
        Error::Api(api) => {
            fields.insert("code".into(), json!(api.code));
            fields.insert("description".into(), json!(api.description));
            fields.insert("retryable".into(), json!(api.retryable));
            fields.insert("taskId".into(), json!(api.task_id));
            fields.insert(
                "supportedUserAgents".into(),
                json!(api.supported_user_agents),
            );
            match api.kind {
                ApiErrorKind::Authentication => "AuthenticationError",
                ApiErrorKind::InvalidTask => "InvalidTaskError",
                ApiErrorKind::InsufficientBalance => "InsufficientBalanceError",
                ApiErrorKind::NoSlotAvailable => "NoSlotAvailableError",
                ApiErrorKind::TaskNotFound => "TaskNotFoundError",
                ApiErrorKind::TaskFailed => "TaskFailedError",
                _ => "ApiError",
            }
        }
        _ => "unexpected error variant",
    };
    (kind.to_owned(), fields)
}

/// The result in wire spelling (serde's snake_case field names; solutions untouched).
fn to_value<T: serde::Serialize>(result: Result<T, Error>) -> Result<Value, Error> {
    result.map(|value| serde_json::to_value(value).unwrap())
}

enum Outcome {
    Ok(Value),
    Err(Error),
    /// The future was dropped (Rust's cancellation).
    Cancelled,
    /// The option value is unrepresentable in the builder's type (a negative u32).
    RejectedByType,
}

async fn run(sc: &Value) -> Result<(), String> {
    let call = &sc["call"];
    let args = &call["args"];
    let expect = &sc["expect"];
    let script = sc
        .get("script")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let state = Arc::new(Mutex::new(ScriptState {
        script: script.clone(),
        requests: vec![],
        failures: vec![],
    }));
    let cancelled = Arc::new(Notify::new());
    let clock = VirtualClock {
        start: Instant::now(),
        state: Arc::new(Mutex::new(ClockState {
            elapsed: Duration::ZERO,
            sleeps: vec![],
        })),
        cancel_at: call
            .get("cancelAfterSleeps")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize,
        cancelled: cancelled.clone(),
    };
    let random = sc.get("random").and_then(Value::as_f64).unwrap_or(0.0);
    let options = sc
        .get("client")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let api_key = options
        .get("apiKey")
        .and_then(Value::as_str)
        .unwrap_or(TEST_API_KEY)
        .to_owned();
    let mut builder = Client::builder(api_key.clone())
        .transport(ScriptedTransport(state.clone()))
        .clock(clock.clone())
        .random(move || random);
    let ms = |v: &Value| Duration::from_millis(v.as_u64().unwrap());
    let mut rejected_by_type = false;
    for (name, value) in &options {
        builder = match name.as_str() {
            "apiKey" => builder,
            "baseUrl" => builder.base_url(value.as_str().unwrap()),
            "requestTimeoutMs" => builder.request_timeout(ms(value)),
            "pollIntervalMs" => builder.poll_interval(ms(value)),
            "solveTimeoutMs" => builder.solve_timeout(ms(value)),
            "maxSubmitRetries" => match u32::try_from(value.as_i64().unwrap()) {
                Ok(retries) => builder.max_submit_retries(retries),
                Err(_) => {
                    rejected_by_type = true;
                    builder
                }
            },
            "appId" => builder.app_id(value.as_str().unwrap()),
            "userAgentSuffix" => builder.user_agent_suffix(value.as_str().unwrap()),
            other => return Err(format!("unknown client option {other}")),
        };
    }

    let created = Arc::new(Mutex::new(Vec::<String>::new()));
    let outcome = if rejected_by_type {
        Outcome::RejectedByType
    } else {
        match builder.build() {
            Err(error) => Outcome::Err(error),
            Ok(client) => {
                let task = args.get("task").cloned().unwrap_or(Value::Null);
                let created_sink = created.clone();
                let call_future = async {
                    let method = call["method"].as_str().unwrap();
                    match method {
                        "construct" => Ok(Value::Null),
                        "createTask" => to_value(client.create_task(&task).await),
                        "getTaskResult" => to_value(
                            client
                                .get_task_result(args["taskId"].as_str().unwrap())
                                .await,
                        ),
                        "getBalance" => to_value(client.get_balance().await),
                        "listTaskTypes" => to_value(client.list_task_types().await),
                        "listServices" => to_value(client.list_services().await),
                        "solve" => {
                            let mut options = SolveOptions::default().on_task_created(move |id| {
                                created_sink.lock().unwrap().push(id.to_owned())
                            });
                            if let Some(v) = args.get("pollIntervalMs") {
                                options = options.poll_interval(ms(v));
                            }
                            if let Some(v) = args.get("solveTimeoutMs") {
                                options = options.solve_timeout(ms(v));
                            }
                            to_value(client.solve_with(&task, options).await)
                        }
                        other => panic!("unknown call {other}"),
                    }
                };
                tokio::select! {
                    result = call_future => match result {
                        Ok(value) => Outcome::Ok(value),
                        Err(error) => Outcome::Err(error),
                    },
                    _ = cancelled.notified() => Outcome::Cancelled,
                }
            }
        }
    };

    let state = state.lock().unwrap();
    if !state.failures.is_empty() {
        return Err(format!("harness: {:?}", state.failures));
    }
    if state.requests.len() != script.len() {
        return Err(format!(
            "consumed {} of {} scripted requests",
            state.requests.len(),
            script.len()
        ));
    }
    if let Some(Value::Array(want)) = expect.get("sleeps") {
        let got = clock.state.lock().unwrap().sleeps.clone();
        let want: Vec<u64> = want.iter().map(|v| v.as_u64().unwrap()).collect();
        if got != want {
            return Err(format!("sleeps {got:?}, want {want:?}"));
        }
    }
    if let Some(last) = expect.get("lastTaskId").and_then(Value::as_str) {
        let created = created.lock().unwrap();
        if created.last().map(String::as_str) != Some(last) {
            return Err(format!("onTaskCreated got {created:?}, want last {last}"));
        }
    }

    let expected_error = expect.get("outcome").and_then(|o| o.get("error"));
    match (&outcome, expected_error) {
        (Outcome::Ok(wire), None) => {
            if let Some(want) = expect.get("outcome").and_then(|o| o.get("ok")) {
                if !json_eq(&normalize(want, false), &normalize(wire, false)) {
                    return Err(format!("result {wire}, want {want}"));
                }
            }
            for check in expect
                .get("outcomeSubset")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let pointer = check["pointer"].as_str().unwrap();
                let value = pointer_get(wire, pointer);
                if let Some(length) = check.get("length").and_then(Value::as_u64) {
                    let got = value.as_array().map_or(0, Vec::len) as u64;
                    if got != length {
                        return Err(format!("{pointer} length {got}, want {length}"));
                    }
                }
                if let Some(want) = check.get("equals") {
                    if !json_eq(&normalize(want, false), &normalize(value, false)) {
                        return Err(format!("{pointer} = {value}, want {want}"));
                    }
                }
            }
        }
        (Outcome::Ok(wire), Some(want)) => {
            return Err(format!("expected {}, got result {wire}", want["type"]));
        }
        (Outcome::Cancelled, Some(want)) if want["type"] == "Cancelled" => {}
        (Outcome::RejectedByType, Some(want)) if want["type"] == "ConfigurationError" => {}
        (Outcome::Cancelled | Outcome::RejectedByType, want) => {
            return Err(format!("cancelled or rejected by type, want {want:?}"));
        }
        (Outcome::Err(error), None) => return Err(format!("unexpected error: {error}")),
        (Outcome::Err(error), Some(want)) => {
            let (kind, fields) = error_view(error);
            if want["type"] != kind.as_str() {
                return Err(format!("error {kind} ({error}), want {}", want["type"]));
            }
            for (name, value) in want.as_object().unwrap() {
                if matches!(
                    name.as_str(),
                    "type" | "messageIncludes" | "messageExcludes"
                ) {
                    continue;
                }
                let got = fields.get(name).unwrap_or(&Value::Null);
                if !json_eq(value, got) {
                    return Err(format!("{name} = {got}, want {value}"));
                }
            }
            let rendered = format!("{error}\n{error:?}");
            for text in want
                .get("messageIncludes")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if !rendered.contains(text.as_str().unwrap()) {
                    return Err(format!("message {rendered:?} lacks {text}"));
                }
            }
            for text in want
                .get("messageExcludes")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if rendered.contains(text.as_str().unwrap()) {
                    return Err(format!("message {rendered:?} contains {text}"));
                }
            }
        }
    }

    if expect.get("redaction") == Some(&Value::Bool(true)) {
        for request in &state.requests {
            let body = String::from_utf8(request.body.clone().unwrap_or_default()).unwrap();
            let parsed: Value = serde_json::from_str(&body).unwrap();
            if request.url.contains(&api_key)
                || request.headers.iter().any(|(_, v)| v.contains(&api_key))
                || body.matches(&api_key).count() != 1
                || parsed["clientKey"] != api_key.as_str()
            {
                return Err("key outside the clientKey body field".into());
            }
        }
        if let Outcome::Err(error) = &outcome {
            if format!("{error} {error:?}").contains(&api_key) {
                return Err("key in error output".into());
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn conformance_scenarios() {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root().join("scenarios"))
        .expect("vendored scenarios")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no vendored scenarios");
    let mut failures = Vec::new();
    for file in &files {
        let sc: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        if let Err(message) = run(&sc).await {
            failures.push(format!("{}: {message}", sc["name"].as_str().unwrap()));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} scenarios failed:\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
    println!("{} scenarios passed", files.len());
}
