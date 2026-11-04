//! Behaviour the language-neutral scenarios cannot cover: the real reqwest transport against a
//! local server, cancellation by dropping the future, and Debug redaction.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use sursolver::{ApiErrorKind, Client, Error, SolveOptions, task_types};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Notify;

const KEY: &str = "sur_00000000000000000000000000000000";
const TASK_ID: &str = "tsk_4f7a9c1e2b8d3a6f5c0e9d21";

#[derive(Debug, Clone)]
struct Recorded {
    line: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Recorded {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

type Handler = Arc<dyn Fn(&Recorded) -> Option<String> + Send + Sync>;

/// A minimal HTTP/1.1 server on 127.0.0.1; the handler returns a raw response, or None to never answer.
async fn serve(handler: Handler) -> (String, Arc<Mutex<Vec<Recorded>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let handler = handler.clone();
            let seen = seen.clone();
            tokio::spawn(async move {
                let mut buffer = Vec::new();
                let mut chunk = [0u8; 4096];
                let head_end = loop {
                    let n = socket.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buffer.extend_from_slice(&chunk[..n]);
                    if let Some(i) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let head = String::from_utf8_lossy(&buffer[..head_end]).to_string();
                let mut lines = head.split("\r\n");
                let line = lines
                    .next()
                    .unwrap()
                    .split(' ')
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(" ");
                let headers: Vec<(String, String)> = lines
                    .filter_map(|l| l.split_once(':'))
                    .map(|(n, v)| (n.trim().to_owned(), v.trim().to_owned()))
                    .collect();
                let length = headers
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case("content-length"))
                    .map_or(0, |(_, v)| v.parse().unwrap());
                while buffer.len() < head_end + length {
                    let n = socket.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buffer.extend_from_slice(&chunk[..n]);
                }
                let body = String::from_utf8_lossy(&buffer[head_end..]).to_string();
                let request = Recorded {
                    line,
                    headers,
                    body,
                };
                seen.lock().unwrap().push(request.clone());
                match handler(&request) {
                    Some(response) => {
                        let _ = socket.write_all(response.as_bytes()).await;
                    }
                    None => std::future::pending::<()>().await,
                }
            });
        }
    });
    (base, requests)
}

fn json_response(body: &str) -> Option<String> {
    Some(format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ))
}

fn task() -> Value {
    json!({
        "type": task_types::TURNSTILE_TOKEN_PROXY_LESS,
        "websiteURL": "https://example.com/",
        "websiteKey": "k",
        "userAgent": "ua",
    })
}

#[tokio::test]
async fn key_travels_only_in_the_body_with_the_sdk_user_agent() {
    let (base, requests) = serve(Arc::new(|_| {
        json_response(&format!(r#"{{"errorId":0,"taskId":"{TASK_ID}"}}"#))
    }))
    .await;
    let client = Client::builder(KEY).base_url(&base).build().unwrap();

    assert_eq!(client.create_task(&task()).await.unwrap(), TASK_ID);

    let requests = requests.lock().unwrap();
    let request = &requests[0];
    assert_eq!(request.line, "POST /createTask");
    assert!(
        request
            .header("user-agent")
            .unwrap()
            .starts_with(&format!("sursolver-rust/{}", sursolver::VERSION))
    );
    assert_eq!(request.header("content-type"), Some("application/json"));
    assert!(request.headers.iter().all(|(_, v)| !v.contains(KEY)));
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["clientKey"], KEY);
}

#[tokio::test]
async fn request_timeout_is_a_transport_error() {
    let (base, _) = serve(Arc::new(|_| None)).await;
    // A real 100 ms wait: the behaviour under test is reqwest's own deadline handling.
    let client = Client::builder(KEY)
        .base_url(&base)
        .request_timeout(Duration::from_millis(100))
        .build()
        .unwrap();

    match client.get_balance().await {
        Err(Error::Transport(error)) => {
            assert!(
                error.to_string().contains("timed out after 100 ms"),
                "{error}"
            );
            assert!(!error.may_have_created_task());
            assert_eq!(error.status, None);
        }
        other => panic!("got {other:?}"),
    }
}

#[tokio::test]
async fn redirect_is_not_followed() {
    let (base, requests) = serve(Arc::new(|_| {
        Some("HTTP/1.1 307 Temporary Redirect\r\nLocation: /elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into())
    }))
    .await;
    let client = Client::builder(KEY).base_url(&base).build().unwrap();

    match client.create_task(&task()).await {
        Err(Error::Transport(error)) => {
            assert_eq!(error.status, Some(307));
            assert!(error.may_have_created_task());
            assert!(error.to_string().contains("charged"));
        }
        other => panic!("got {other:?}"),
    }
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn dropping_the_future_during_polling_keeps_the_task_id() {
    let polled = Arc::new(Notify::new());
    let signal = polled.clone();
    let (base, _) = serve(Arc::new(move |request| {
        if request.line == "POST /createTask" {
            json_response(&format!(r#"{{"errorId":0,"taskId":"{TASK_ID}"}}"#))
        } else {
            signal.notify_one();
            json_response(r#"{"errorId":0,"status":"processing"}"#)
        }
    }))
    .await;
    let client = Client::builder(KEY)
        .base_url(&base)
        .poll_interval(Duration::from_secs(10))
        .build()
        .unwrap();
    let created = Mutex::new(Vec::new());

    let task = task();
    let solve = client.solve_with(
        &task,
        SolveOptions::default().on_task_created(|id| created.lock().unwrap().push(id.to_owned())),
    );
    tokio::select! {
        result = solve => panic!("solve finished: {result:?}"),
        _ = polled.notified() => {} // the solve future is dropped here, inside its 10 s wait or the poll
    }

    assert_eq!(*created.lock().unwrap(), vec![TASK_ID.to_owned()]);
}

#[tokio::test]
async fn api_errors_and_debug_never_show_the_key() {
    let (base, _) = serve(Arc::new(|_| {
        json_response(r#"{"errorId":1,"errorCode":"ERROR_KEY_DOES_NOT_EXIST","errorDescription":"The clientKey does not match any active API key."}"#)
    }))
    .await;
    let client = Client::builder(KEY).base_url(&base).build().unwrap();

    let error = client.get_balance().await.unwrap_err();

    assert_eq!(error.api_kind(), Some(ApiErrorKind::Authentication));
    assert!(!format!("{error} {error:?}").contains(KEY));
    assert!(!format!("{client:?}").contains(KEY));
    assert!(format!("{client:?}").contains(&base));
}
