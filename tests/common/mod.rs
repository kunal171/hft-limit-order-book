use axum::{
    Router,
    body::{Body, to_bytes},
    http::{
        Method, Request, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    response::Response,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tower::ServiceExt;

const TEST_PASSWORD: &str = "correct-horse-battery-staple";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// Only the login field needed by these tests.
#[derive(Deserialize)]
struct LoginBody {
    access_token: String,
}

/// Builds a JSON request with an optional bearer token.
pub fn json_request(method: Method, uri: &str, body: Value, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(CONTENT_TYPE, "application/json");

    if let Some(token) = token {
        builder = builder.header(AUTHORIZATION, format!("Bearer {token}"));
    }

    builder
        .body(Body::from(body.to_string()))
        .expect("test request should be valid")
}

/// Builds a request without a JSON body.
// Some integration-test binaries use only the JSON request helper.
#[allow(dead_code)]
pub fn empty_request(method: Method, uri: &str, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);

    if let Some(token) = token {
        builder = builder.header(AUTHORIZATION, format!("Bearer {token}"));
    }

    builder
        .body(Body::empty())
        .expect("test request should be valid")
}

/// Sends one request through the real Axum router without opening a TCP port.
pub async fn send(app: &Router, request: Request<Body>) -> Response {
    app.clone()
        .oneshot(request)
        .await
        .expect("router should return a response")
}

/// Creates a normal trader through the public API.
pub async fn signup(app: &Router, email: &str) {
    let response = send(
        app,
        json_request(
            Method::POST,
            "/auth/signup",
            json!({
                "display_name": "Integration Test Trader",
                "email": email,
                "password": TEST_PASSWORD,
            }),
            None,
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
}

/// Logs in and returns the raw token that a client would retain.
pub async fn login(app: &Router, email: &str) -> String {
    let response = send(
        app,
        json_request(
            Method::POST,
            "/auth/login",
            json!({
                "email": email,
                "password": TEST_PASSWORD,
            }),
            None,
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), MAX_RESPONSE_BYTES)
        .await
        .expect("login body should be readable");
    let login: LoginBody =
        serde_json::from_slice(&body).expect("login response should contain JSON");

    login.access_token
}
