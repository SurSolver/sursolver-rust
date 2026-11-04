use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A task's result. Keys differ per type (`token`, `gRecaptchaResponse`, `cf_clearance`,
/// `cookies`, ...) and are passed through untouched. Tokens are single-use.
pub type Solution = Map<String, Value>;

/// One `getTaskResult` answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", content = "solution", rename_all = "lowercase")]
pub enum TaskResult {
    /// The task is still running.
    Processing,
    /// The task is solved.
    Ready(Solution),
}

/// What [`Client::solve`](crate::Client::solve) returns.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SolveResult {
    /// The solved task.
    pub task_id: String,
    /// The solution, passed through untouched. Tokens and cookies are single-use.
    pub solution: Solution,
}

/// One entry of `GET /v1/task-types`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TaskTypeInfo {
    /// The solver page slug at `https://sursolver.com/solvers/<slug>`, when the type is active.
    pub page_slug: Option<String>,
    /// Human-readable name.
    pub display_name: String,
    /// Canonical `task.type`.
    pub task_type: String,
    /// The ProxyLess twin, when there is one.
    pub task_type_proxyless: Option<String>,
    /// Other accepted spellings.
    pub aliases: Vec<String>,
    /// `none`, `optional` or `required`.
    pub proxy: String,
    /// Groups of accepted spellings; supply one name per group.
    pub required: Vec<Vec<String>>,
    /// Alternatives; supply every field of at least one.
    pub any_of: Vec<Vec<String>>,
    /// Optional fields.
    pub optional: Vec<String>,
    /// Keys of the solution object.
    pub solution_keys: Vec<String>,
}

/// The public task-type catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TaskTypeCatalog {
    /// Every task type.
    pub task_types: Vec<TaskTypeInfo>,
    /// Every `errorCode` the solver endpoints send.
    pub error_codes: Vec<String>,
}

/// One entry of `GET /v1/services`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ServiceInfo {
    /// Service id.
    pub id: i64,
    /// Service slug.
    pub slug: String,
    /// Human-readable name.
    pub display_name: String,
    /// USD price per task as a decimal string, e.g. `"0.0006"`.
    pub price_per_request: String,
    /// Service status.
    pub status: String,
    /// Recent average latency in milliseconds, when known.
    pub avg_latency_ms: Option<i64>,
}
