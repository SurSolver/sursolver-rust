//! Official Rust client for the [SurSolver](https://sursolver.com) captcha-solving API.
//!
//! ```no_run
//! use serde_json::json;
//! use sursolver::{Client, task_types};
//!
//! # async fn run() -> Result<(), sursolver::Error> {
//! let client = Client::new("YOUR_API_KEY")?;
//! let result = client
//!     .solve(&json!({
//!         "type": task_types::TURNSTILE_TOKEN_PROXY_LESS,
//!         "websiteURL": "https://example.com/login",
//!         "websiteKey": "0x4AAAAAAA...",
//!         "userAgent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) ...",
//!     }))
//!     .await?;
//! println!("{}", result.solution["token"]);
//! # Ok(())
//! # }
//! ```
//!
//! The API key travels only in request bodies and never appears in errors or `Debug` output.
#![warn(missing_docs)]

mod client;
mod error;
mod generated;
mod transport;
mod types;

pub use client::{
    Client, ClientBuilder, DEFAULT_BASE_URL, DEFAULT_MAX_SUBMIT_RETRIES, DEFAULT_POLL_INTERVAL,
    DEFAULT_REQUEST_TIMEOUT, DEFAULT_SOLVE_TIMEOUT, SUBMIT_RETRY_JITTER, SolveOptions, VERSION,
};
pub use error::{ApiError, ApiErrorKind, Error, Operation, SolveTimeoutError, TransportError};
pub use generated::{error_codes, task_types};
pub use transport::{
    BoxFuture, Clock, Request, ReqwestTransport, Response, Transport, TransportFailure,
};
pub use types::{ServiceInfo, Solution, SolveResult, TaskResult, TaskTypeCatalog, TaskTypeInfo};
