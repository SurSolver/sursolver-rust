use std::fmt;
use std::time::Duration;

use crate::error_codes;

/// The client call an error belongs to; [`Client::solve`](crate::Client::solve) reports the inner
/// call that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operation {
    /// `createTask`
    CreateTask,
    /// `getTaskResult`
    GetTaskResult,
    /// `getBalance`
    GetBalance,
    /// `GET /v1/task-types`
    ListTaskTypes,
    /// `GET /v1/services`
    ListServices,
}

impl Operation {
    /// The operation's name in the API documentation, e.g. `createTask`.
    pub fn as_str(self) -> &'static str {
        match self {
            Operation::CreateTask => "createTask",
            Operation::GetTaskResult => "getTaskResult",
            Operation::GetBalance => "getBalance",
            Operation::ListTaskTypes => "listTaskTypes",
            Operation::ListServices => "listServices",
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What an [`ApiError`] means for the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ApiErrorKind {
    /// `ERROR_INTERNAL_ERROR`, `ERROR_FAILED` on create, or a code newer than this crate.
    Api,
    /// `ERROR_WRONG_CLIENT_KEY` or `ERROR_KEY_DOES_NOT_EXIST`: fix the API key.
    Authentication,
    /// `ERROR_INVALID_REQUEST`, `ERROR_TYPE_NOT_SUPPORTED`, or `ERROR_WRONG_USERAGENT` on create
    /// (see [`ApiError::supported_user_agents`]). Nothing was created or charged.
    InvalidTask,
    /// `ERROR_ZERO_BALANCE`: top up. Nothing was charged.
    InsufficientBalance,
    /// `ERROR_NO_SLOT_AVAILABLE`: all your threads are busy. Nothing was created; submitting later
    /// is safe.
    NoSlotAvailable,
    /// `ERROR_TASK_NOT_FOUND`.
    TaskNotFound,
    /// The task ended without a result (`ERROR_FAILED`, `ERROR_TASK_TIMEOUT`, `ERROR_TASK_EXPIRED`,
    /// or `ERROR_WRONG_USERAGENT` after acceptance). Its charge is returned once.
    TaskFailed,
}

/// The API answered `errorId: 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ApiError {
    /// What the error means for the caller.
    pub kind: ApiErrorKind,
    /// The call that failed.
    pub operation: Operation,
    /// The `errorCode` exactly as sent, including codes newer than this crate.
    pub code: String,
    /// The `errorDescription` as sent.
    pub description: String,
    /// True only for `ERROR_NO_SLOT_AVAILABLE`.
    pub retryable: bool,
    /// Set for `getTaskResult` and for `solve` after the task was accepted.
    pub task_id: Option<String>,
    /// The list sent with `ERROR_WRONG_USERAGENT`; empty otherwise.
    pub supported_user_agents: Vec<String>,
}

impl ApiError {
    pub(crate) fn new(
        operation: Operation,
        code: String,
        description: String,
        task_id: Option<String>,
        supported_user_agents: Vec<String>,
    ) -> Self {
        let task_failure = matches!(
            code.as_str(),
            error_codes::ERROR_FAILED
                | error_codes::ERROR_TASK_TIMEOUT
                | error_codes::ERROR_TASK_EXPIRED
                | error_codes::ERROR_WRONG_USERAGENT
        );
        let kind = if operation == Operation::GetTaskResult && task_failure {
            ApiErrorKind::TaskFailed
        } else {
            match code.as_str() {
                error_codes::ERROR_WRONG_CLIENT_KEY | error_codes::ERROR_KEY_DOES_NOT_EXIST => {
                    ApiErrorKind::Authentication
                }
                error_codes::ERROR_INVALID_REQUEST
                | error_codes::ERROR_TYPE_NOT_SUPPORTED
                | error_codes::ERROR_WRONG_USERAGENT => ApiErrorKind::InvalidTask,
                error_codes::ERROR_ZERO_BALANCE => ApiErrorKind::InsufficientBalance,
                error_codes::ERROR_NO_SLOT_AVAILABLE => ApiErrorKind::NoSlotAvailable,
                error_codes::ERROR_TASK_NOT_FOUND => ApiErrorKind::TaskNotFound,
                _ => ApiErrorKind::Api,
            }
        };
        ApiError {
            kind,
            operation,
            retryable: code == error_codes::ERROR_NO_SLOT_AVAILABLE,
            code,
            description,
            task_id,
            supported_user_agents,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.description)?;
        if let Some(task_id) = &self.task_id {
            write!(f, " (task {task_id})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}

/// No usable answer: a connection failure, a timeout, a non-200 status, or a body that is not the
/// API's envelope. It says nothing about the task.
#[derive(Debug)]
#[non_exhaustive]
pub struct TransportError {
    /// The call that failed.
    pub operation: Operation,
    /// What went wrong, e.g. `HTTP 503`.
    pub reason: String,
    /// The HTTP status when a response arrived.
    pub status: Option<u16>,
    /// `error.code` of the API's internal envelope, when present.
    pub server_code: Option<String>,
    /// `error.retry_after_seconds` of the internal envelope, when present.
    pub retry_after_seconds: Option<i64>,
    pub(crate) source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl TransportError {
    /// True for `createTask`: the request may have been accepted and charged before the answer was
    /// lost. Check [`Client::get_balance`](crate::Client::get_balance) before submitting again.
    pub fn may_have_created_task(&self) -> bool {
        self.operation == Operation::CreateTask
    }
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} failed: {}.", self.operation, self.reason)?;
        if self.may_have_created_task() {
            f.write_str(
                " The task may have been created and charged; check get_balance() before submitting again.",
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|e| e as &(dyn std::error::Error + 'static))
    }
}

/// [`Client::solve`](crate::Client::solve) reached its deadline. The task may still finish and stays
/// charged; poll it with `get_task_result(&task_id)` if you still need the result.
#[derive(Debug)]
#[non_exhaustive]
pub struct SolveTimeoutError {
    /// The task that was not ready in time.
    pub task_id: String,
    /// The deadline that passed.
    pub timeout: Duration,
    /// The final poll's transient failure, when there was one.
    pub last_error: Option<Box<Error>>,
}

impl fmt::Display for SolveTimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "task {} was not ready within {:?}; it may still finish and stays charged",
            self.task_id, self.timeout
        )
    }
}

impl std::error::Error for SolveTimeoutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.last_error
            .as_deref()
            .map(|e| e as &(dyn std::error::Error + 'static))
    }
}

/// Every error the client returns. Messages never contain the API key. Cancellation is dropping the
/// future, so it has no variant.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The client was built with an unusable option. Nothing was sent.
    Configuration(String),
    /// No usable answer; see [`TransportError`].
    Transport(TransportError),
    /// HTTP 200 JSON whose shape the API contract does not allow.
    Protocol {
        /// The call that failed.
        operation: Operation,
        /// What is wrong with the response.
        detail: String,
    },
    /// `solve` reached its deadline; see [`SolveTimeoutError`].
    SolveTimeout(SolveTimeoutError),
    /// The API answered `errorId: 1`; see [`ApiError`].
    Api(ApiError),
}

impl Error {
    /// The call that failed; `None` for configuration errors and solve timeouts.
    pub fn operation(&self) -> Option<Operation> {
        match self {
            Error::Transport(e) => Some(e.operation),
            Error::Protocol { operation, .. } => Some(*operation),
            Error::Api(e) => Some(e.operation),
            Error::Configuration(_) | Error::SolveTimeout(_) => None,
        }
    }

    /// The kind of an API error, `None` for every other error.
    pub fn api_kind(&self) -> Option<ApiErrorKind> {
        match self {
            Error::Api(e) => Some(e.kind),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Configuration(message) => f.write_str(message),
            Error::Transport(e) => e.fmt(f),
            Error::Protocol { operation, detail } => {
                write!(f, "{operation} returned an unexpected response: {detail}")
            }
            Error::SolveTimeout(e) => e.fmt(f),
            Error::Api(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Transport(e) => e.source(),
            Error::SolveTimeout(e) => e.source(),
            _ => None,
        }
    }
}
