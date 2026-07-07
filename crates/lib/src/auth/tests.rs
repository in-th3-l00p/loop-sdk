/* end-to-end auth flows over a real router and the global in-memory
database. this module owns the global database + auth singletons for the
whole test binary (eth/tests.rs owns the eth one) */

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value as Json, json};
use tower::ServiceExt;

use super::config::{Config, Provider};
use super::otp::Mailer;
use super::error::AuthError;

static CODES: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn codes() -> &'static Mutex<HashMap<String, String>> {
    CODES.get_or_init(Mutex::default)
}

/// Captures one-time codes instead of printing them.
struct TestMailer;

impl Mailer for TestMailer {
    fn send(&self, to: &str, _from: Option<&str>, code: &str) -> Result<(), AuthError> {
        codes().lock().unwrap().insert(to.to_string(), code.to_string());
        Ok(())
    }
}

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static SETUP: OnceLock<()> = OnceLock::new();

fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().expect("test runtime"))
}

fn test_config() -> Config {
    let mut config = Config::new(vec![Provider::EmailPassword, Provider::EmailOtp]);
    config.session_ttl = Duration::from_secs(3600);
    config
}

fn setup() {
    SETUP.get_or_init(|| {
        super::set_mailer(TestMailer);
        runtime().block_on(async {
            crate::database::init(&crate::database::Config::from_url("sqlite::memory:"), &[])
                .await
                .expect("global test database");
            super::init(test_config()).await.expect("auth init");
        });
    });
}

fn router() -> Router {
    setup();
    let engine =
        crate::server::endpoint::engine::Engine::new(super::endpoints(&test_config())).unwrap();
    crate::server::router(&engine)
}

async fn send(router: Router, request: Request<Body>) -> (StatusCode, Json) {
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Json::Null))
}

fn post(uri: &str, body: Json, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

fn get(uri: &str, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().uri(uri);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::empty()).unwrap()
}

#[test]
fn register_returns_a_session_and_rejects_duplicates_and_junk() {
    let app = router();
    runtime().block_on(async {
        let credentials = json!({"email": "Ada@Example.COM", "password": "hunter2222"});
        let (status, body) = send(app.clone(), post("/auth/register", credentials.clone(), None)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["user"]["email"], json!("ada@example.com"));
        assert_eq!(body["user"]["wallets"], json!([]));
        assert!(body["token"].as_str().unwrap().len() == 64);

        // same email, case-insensitively → 409
        let (status, _) = send(app.clone(), post("/auth/register", credentials, None)).await;
        assert_eq!(status, StatusCode::CONFLICT);

        let weak = json!({"email": "b@example.com", "password": "short"});
        let (status, _) = send(app.clone(), post("/auth/register", weak, None)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let junk = json!({"email": "not-an-email", "password": "hunter2222"});
        let (status, _) = send(app.clone(), post("/auth/register", junk, None)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    });
}

#[test]
fn login_checks_credentials() {
    let app = router();
    runtime().block_on(async {
        let credentials = json!({"email": "login@example.com", "password": "hunter2222"});
        let (status, _) = send(app.clone(), post("/auth/register", credentials.clone(), None)).await;
        assert_eq!(status, StatusCode::OK);

        let (status, body) = send(app.clone(), post("/auth/login", credentials, None)).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["token"].as_str().is_some());

        let wrong = json!({"email": "login@example.com", "password": "wrong-password"});
        let (status, _) = send(app.clone(), post("/auth/login", wrong, None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let unknown = json!({"email": "nobody@example.com", "password": "hunter2222"});
        let (status, _) = send(app.clone(), post("/auth/login", unknown, None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    });
}

#[test]
fn sessions_introspect_and_revoke() {
    let app = router();
    runtime().block_on(async {
        let credentials = json!({"email": "session@example.com", "password": "hunter2222"});
        let (_, body) = send(app.clone(), post("/auth/register", credentials, None)).await;
        let token = body["token"].as_str().unwrap().to_string();

        let (status, session) = send(app.clone(), get("/auth/session", Some(&token))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(session["email"], json!("session@example.com"));

        let (status, _) = send(app.clone(), get("/auth/session", None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _) = send(app.clone(), get("/auth/session", Some("forged"))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, body) = send(app.clone(), post("/auth/logout", json!({}), Some(&token))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({"ok": true}));

        let (status, _) = send(app.clone(), get("/auth/session", Some(&token))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "revoked token must die");
    });
}

#[test]
fn otp_codes_login_once_and_register_on_first_use() {
    let app = router();
    runtime().block_on(async {
        let email = "otp@example.com";
        let (status, body) =
            send(app.clone(), post("/auth/otp/send", json!({"email": email}), None)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let code = codes().lock().unwrap().get(email).unwrap().clone();

        let wrong = json!({"email": email, "code": "000000"});
        let (status, _) = send(app.clone(), post("/auth/otp/verify", wrong, None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let right = json!({"email": email, "code": code});
        let (status, body) = send(app.clone(), post("/auth/otp/verify", right.clone(), None)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["user"]["email"], json!(email));

        // single use: the same code cannot mint a second session
        let (status, _) = send(app.clone(), post("/auth/otp/verify", right, None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    });
}

#[test]
fn otp_codes_lock_after_too_many_guesses() {
    let app = router();
    runtime().block_on(async {
        let email = "bruteforce@example.com";
        send(app.clone(), post("/auth/otp/send", json!({"email": email}), None)).await;
        let code = codes().lock().unwrap().get(email).unwrap().clone();

        for _ in 0..5 {
            let guess = json!({"email": email, "code": "999999"});
            let (status, _) = send(app.clone(), post("/auth/otp/verify", guess, None)).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
        }
        // even the right code is dead now
        let right = json!({"email": email, "code": code});
        let (status, _) = send(app.clone(), post("/auth/otp/verify", right, None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    });
}

#[test]
fn user_params_resolve_sessions_through_from_context() {
    use crate::server::endpoint::{Context, FromContext};

    let app = router();
    let token = runtime().block_on(async {
        let credentials = json!({"email": "ctx@example.com", "password": "hunter2222"});
        let (_, body) = send(app.clone(), post("/auth/register", credentials, None)).await;
        body["token"].as_str().unwrap().to_string()
    });

    // resolution happens off the runtime, exactly like handler threads do
    let user = super::User::from_context(&Context::with_token(&token)).unwrap();
    assert_eq!(user.email().as_deref(), Some("ctx@example.com"));
    assert_eq!(
        super::users()
            .by_email("ctx@example.com")
            .unwrap()
            .unwrap()
            .id(),
        user.id()
    );

    let anonymous = super::User::from_context(&Context::default());
    assert!(anonymous.is_err());
    let optional = Option::<super::User>::from_context(&Context::default()).unwrap();
    assert!(optional.is_none());
    let optional = Option::<super::User>::from_context(&Context::with_token(&token)).unwrap();
    assert_eq!(optional.unwrap().id(), user.id());
}
