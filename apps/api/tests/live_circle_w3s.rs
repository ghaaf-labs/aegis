//! Live smoke for Circle W3S API access.
//!
//! Hits `GET /v1/w3s/config/entity` against `https://api.circle.com` with the
//! current `CIRCLE_API_KEY` from the environment and asserts a 200 response.
//! A 401 here means the key isn't W3S-enabled — go to Circle Console → Keys
//! → Create a Standard Key with the Wallets product enabled.
//!
//! `#[ignore]` by default — run with `cargo test --test live_circle_w3s -- --ignored`.

#[tokio::test]
#[ignore]
async fn circle_w3s_key_has_wallets_access() {
    let key = std::env::var("CIRCLE_API_KEY").unwrap_or_default();
    if key.is_empty() {
        eprintln!("skipping: CIRCLE_API_KEY not set");
        return;
    }
    let http = reqwest::Client::builder()
        .user_agent("aegis-live-smoke/0.1.0")
        .build()
        .unwrap();
    let resp = http
        .get("https://api.circle.com/v1/w3s/config/entity")
        .header("Authorization", format!("Bearer {key}"))
        .send()
        .await
        .expect("network ok");
    assert_eq!(
        resp.status().as_u16(),
        200,
        "Circle W3S config/entity returned {} — key likely lacks Wallets product access",
        resp.status()
    );
}
