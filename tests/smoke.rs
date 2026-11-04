//! Live smoke test against the real API; costs nothing. Run: cargo test --test smoke -- --ignored --nocapture
//! It reads the public catalog and calls get_balance with a fake key (expects an authentication
//! error). Set SURSOLVER_API_KEY to also read your real balance; SURSOLVER_BASE_URL overrides the origin.

use sursolver::{ApiErrorKind, Client, DEFAULT_BASE_URL};

#[tokio::test]
#[ignore = "talks to the live API"]
async fn live_catalog_and_fake_key() {
    let base_url =
        std::env::var("SURSOLVER_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_owned());
    let client = Client::builder("sur_00000000000000000000000000000000")
        .base_url(&base_url)
        .build()
        .unwrap();

    let catalog = client.list_task_types().await.unwrap();
    println!(
        "list_task_types: {} types, {} error codes",
        catalog.task_types.len(),
        catalog.error_codes.len()
    );
    let services = client.list_services().await.unwrap();
    println!("list_services: {} active services", services.len());
    let error = client.get_balance().await.unwrap_err();
    assert_eq!(
        error.api_kind(),
        Some(ApiErrorKind::Authentication),
        "{error}"
    );
    println!("get_balance (fake key): {error}");

    if let Ok(api_key) = std::env::var("SURSOLVER_API_KEY") {
        let balance = Client::builder(api_key)
            .base_url(&base_url)
            .build()
            .unwrap()
            .get_balance()
            .await
            .unwrap();
        println!("get_balance (your key): {balance} USD");
    }
}
