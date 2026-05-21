use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use polymarket_backend::{config::AppConfig, router::build_router};
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn healthz_returns_structured_service_status() {
    let config = AppConfig::from_env_map([
        ("POLY_ENV".to_string(), "local".to_string()),
        ("APP_VERSION".to_string(), "test-version".to_string()),
        ("SUPPORTED_MARKETS".to_string(), "btc5m,eth15m".to_string()),
    ])
    .expect("test config should be valid");
    let app = build_router(config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("health request should be handled");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    let json: Value = serde_json::from_slice(&body).expect("response should be json");

    assert_eq!(
        json,
        json!({
            "status": "ok",
            "service": "polymarket-backend",
            "version": "test-version",
            "build": {
                "version": "test-version"
            },
            "environment": "local",
            "database_configured": false,
            "supported_markets": ["btc5m", "eth15m"],
            "runtime": {
                "environment": "local",
                "database_configured": false,
                "supported_markets": ["btc5m", "eth15m"]
            }
        })
    );
}
