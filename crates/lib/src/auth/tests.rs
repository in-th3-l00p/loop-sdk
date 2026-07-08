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
    // wallet tests need a master secret; embedded stays off so email flows
    // don't auto-provision wallets in the shared database
    config.secret = Some("42".repeat(32));
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

#[cfg(feature = "eth")]
mod wallets {
    use super::*;
    use crate::auth::wallet::{decrypt_key, encrypt_key};
    use crate::auth::{WalletKind, wallet};
    use crate::eth::Signer as _;

    /// A router that also mounts the wallet routes.
    fn wallet_router() -> Router {
        setup();
        let mut config = test_config();
        config.providers.push(Provider::Wallet);
        let engine =
            crate::server::endpoint::engine::Engine::new(crate::auth::endpoints(&config)).unwrap();
        crate::server::router(&engine)
    }

    /// Signs the SIWE message like a browser wallet would.
    fn personal_sign(signer: &alloy::signers::local::PrivateKeySigner, message: &str) -> String {
        use alloy::signers::SignerSync;
        let signature = signer.sign_message_sync(message.as_bytes()).unwrap();
        crate::auth::crypto::to_hex(&signature.as_bytes())
    }

    #[test]
    fn embedded_keys_roundtrip_and_bind_to_their_user() {
        setup();
        let encrypted = encrypt_key("user-a", "aabbccdd").unwrap();
        assert_eq!(decrypt_key("user-a", &encrypted).unwrap(), "aabbccdd");
        // the per-user derivation means another user's id cannot open it
        assert!(decrypt_key("user-b", &encrypted).is_err());
        // fresh nonces: same plaintext, different ciphertext
        assert_ne!(encrypted, encrypt_key("user-a", "aabbccdd").unwrap());
    }

    #[test]
    fn embedded_wallets_sign_and_export_linked_wallets_refuse() {
        setup();
        let row = crate::auth::store::create_user(None, None).unwrap();
        let embedded = wallet::create_embedded(&row.id).unwrap();
        assert_eq!(embedded.kind(), WalletKind::Embedded);

        // EIP-191 signature recovers to the wallet's own address
        let signature = embedded.sign_message(b"prove it").unwrap();
        let recovered = alloy::primitives::Signature::from_raw(&signature)
            .unwrap()
            .recover_address_from_msg(b"prove it")
            .unwrap();
        assert_eq!(recovered, embedded.address().0);

        // export hands back a key that re-derives the same address
        let exported = embedded.export().unwrap();
        let signer: alloy::signers::local::PrivateKeySigner = exported.parse().unwrap();
        assert_eq!(signer.address(), embedded.address().0);

        // a linked wallet must refuse server-side signing, loudly
        let external = alloy::signers::local::PrivateKeySigner::random();
        let user_id = crate::auth::UserId(row.id.clone());
        wallet::link(&user_id, crate::eth::Address(external.address())).unwrap();
        let linked = crate::auth::users()
            .find(user_id)
            .unwrap()
            .unwrap()
            .wallets()
            .into_iter()
            .find(|w| w.kind() == WalletKind::Linked)
            .unwrap();
        let request = crate::eth::TxRequest(Default::default());
        let error = linked.sign(&request).unwrap_err();
        assert!(error.to_string().contains("self-custodial"), "{error}");
        assert!(linked.export().is_err());
    }

    #[test]
    fn siwe_logs_in_registers_and_links() {
        let app = wallet_router();
        runtime().block_on(async {
            let signer = alloy::signers::local::PrivateKeySigner::random();
            let address = crate::eth::Address(signer.address()).to_string();

            // nonce → sign → verify: registers on first use
            let (status, issued) = send(
                app.clone(),
                get(&format!("/auth/wallet/nonce?address={address}"), None),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{issued}");
            let message = issued["message"].as_str().unwrap();
            assert!(message.contains(&address), "siwe message names the signer");
            let body = json!({
                "address": address,
                "signature": personal_sign(&signer, message),
                "nonce": issued["nonce"],
            });
            let (status, session) =
                send(app.clone(), post("/auth/wallet/verify", body, None)).await;
            assert_eq!(status, StatusCode::OK, "{session}");
            assert_eq!(session["user"]["wallets"][0]["kind"], json!("linked"));
            assert_eq!(session["user"]["wallets"][0]["address"], json!(address));
            let first_user = session["user"]["id"].clone();

            // logging in again with the same wallet reuses the account
            let (_, issued) = send(
                app.clone(),
                get(&format!("/auth/wallet/nonce?address={address}"), None),
            )
            .await;
            let message = issued["message"].as_str().unwrap();
            let body = json!({
                "address": address,
                "signature": personal_sign(&signer, message),
                "nonce": issued["nonce"],
            });
            let (status, again) = send(app.clone(), post("/auth/wallet/verify", body, None)).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(again["user"]["id"], first_user);

            // a nonce is single-use
            let body = json!({
                "address": address,
                "signature": personal_sign(&signer, message),
                "nonce": issued["nonce"],
            });
            let (status, _) = send(app.clone(), post("/auth/wallet/verify", body, None)).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);

            // with a live session, verify links instead of logging in
            let credentials = json!({"email": "linker@example.com", "password": "hunter2222"});
            let (_, registered) = send(app.clone(), post("/auth/register", credentials, None)).await;
            let token = registered["token"].as_str().unwrap().to_string();
            let linker = alloy::signers::local::PrivateKeySigner::random();
            let linker_address = crate::eth::Address(linker.address()).to_string();
            let (_, issued) = send(
                app.clone(),
                get(&format!("/auth/wallet/nonce?address={linker_address}"), None),
            )
            .await;
            let message = issued["message"].as_str().unwrap();
            let body = json!({
                "address": linker_address,
                "signature": personal_sign(&linker, message),
                "nonce": issued["nonce"],
            });
            let (status, linked) =
                send(app.clone(), post("/auth/wallet/verify", body, Some(&token))).await;
            assert_eq!(status, StatusCode::OK, "{linked}");
            assert_eq!(linked["user"]["email"], json!("linker@example.com"));
            assert_eq!(
                linked["user"]["wallets"][0]["address"],
                json!(linker_address)
            );

            // and someone else's session cannot claim that same wallet
            let thief = json!({"email": "thief@example.com", "password": "hunter2222"});
            let (_, registered) = send(app.clone(), post("/auth/register", thief, None)).await;
            let thief_token = registered["token"].as_str().unwrap().to_string();
            let (_, issued) = send(
                app.clone(),
                get(&format!("/auth/wallet/nonce?address={linker_address}"), None),
            )
            .await;
            let message = issued["message"].as_str().unwrap();
            let body = json!({
                "address": linker_address,
                "signature": personal_sign(&linker, message),
                "nonce": issued["nonce"],
            });
            let (status, _) = send(
                app.clone(),
                post("/auth/wallet/verify", body, Some(&thief_token)),
            )
            .await;
            assert_eq!(status, StatusCode::CONFLICT);
        });
    }

    #[test]
    fn siwe_rejects_forged_and_mismatched_signatures() {
        let app = wallet_router();
        runtime().block_on(async {
            let signer = alloy::signers::local::PrivateKeySigner::random();
            let address = crate::eth::Address(signer.address()).to_string();
            let (_, issued) = send(
                app.clone(),
                get(&format!("/auth/wallet/nonce?address={address}"), None),
            )
            .await;
            let message = issued["message"].as_str().unwrap();

            // signed by a different key
            let imposter = alloy::signers::local::PrivateKeySigner::random();
            let body = json!({
                "address": address,
                "signature": personal_sign(&imposter, message),
                "nonce": issued["nonce"],
            });
            let (status, _) = send(app.clone(), post("/auth/wallet/verify", body, None)).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);

            // unknown nonce
            let body = json!({
                "address": address,
                "signature": personal_sign(&signer, message),
                "nonce": "deadbeef",
            });
            let (status, _) = send(app.clone(), post("/auth/wallet/verify", body, None)).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
        });
    }
}
