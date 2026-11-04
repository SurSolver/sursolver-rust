# Changelog

## 0.1.0 — unreleased

First release. Tested against the SDK specification revision in `SPEC_VERSION` (catalog of
2026-11-04).

- `Client` with `create_task`, `get_task_result`, `get_balance`, `list_task_types`,
  `list_services`, `solve` and `solve_with` (3 s polling, 210 s deadline, `on_task_created`;
  cancel by dropping the future).
- `Error` with `Api(ApiError)` (an `ApiErrorKind` per code family and `supported_user_agents`),
  `Transport` (with `may_have_created_task()`), `Protocol`, `SolveTimeout` and `Configuration`.
- `task_types` and `error_codes` constants generated from the public catalog.
- Tokio + reqwest; TLS through rustls by default, `native-tls` as an alternative feature.
  MSRV 1.85.
