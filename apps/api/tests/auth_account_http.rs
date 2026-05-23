use std::{net::SocketAddr, path::Path};

use aegis_api::{
    config::Config,
    modules::{
        sse,
        wallet::{provider::MockProvider, service::WalletService},
    },
    router,
};
use reqwest::StatusCode;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

#[tokio::test]
async fn auth_start_requires_csrf_and_keeps_entry_enumeration_safe() {
    let Some(ctx) = TestContext::start().await else {
        return;
    };
    let known_email = unique_email("known");
    seed_user(&ctx.pool, &known_email, "2026-05", "2026-05").await;

    let missing_csrf = ctx
        .client
        .post(ctx.url("/auth/email/start"))
        .json(&json!({ "email": known_email }))
        .send()
        .await
        .expect("request without csrf");
    assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);
    assert_eq!(error_code(missing_csrf).await, "csrf_failed");

    let known = start_code_response(&ctx, &known_email).await;
    let unknown = start_code_response(&ctx, &unique_email("unknown")).await;
    assert_eq!(known.status(), StatusCode::OK);
    assert_eq!(unknown.status(), StatusCode::OK);

    let mut known_body: Value = known.json().await.expect("known body");
    let mut unknown_body: Value = unknown.json().await.expect("unknown body");
    strip_challenge_specific_fields(&mut known_body);
    strip_challenge_specific_fields(&mut unknown_body);
    assert_eq!(known_body, unknown_body);
}

#[tokio::test]
async fn email_verify_rotates_session_and_revokes_previous_session() {
    let Some(ctx) = TestContext::start().await else {
        return;
    };
    let email = unique_email("rotate");
    let first = complete_email_auth(&ctx, &email).await;
    let second = complete_email_auth(&ctx, &email).await;

    assert_ne!(first.cookie, second.cookie);
    let old_session = session_id_from_cookie(&first.cookie);
    let revoked_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT revoked_at FROM auth_sessions WHERE id = $1")
            .bind(old_session)
            .fetch_one(&ctx.pool)
            .await
            .expect("old session row");
    assert!(revoked_at.is_some());
}

#[tokio::test]
async fn stale_terms_or_privacy_version_requires_reconsent_before_login() {
    let Some(ctx) = TestContext::start().await else {
        return;
    };
    let email = unique_email("stale-consent");
    seed_user(&ctx.pool, &email, "2026-04", "2026-04").await;
    let (challenge_id, code) = start_code(&ctx, &email).await;

    let rejected = verify_code(&ctx, &challenge_id, &code, None, None).await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error_code(rejected).await, "consent_required");

    let accepted = verify_code(&ctx, &challenge_id, &code, Some(current_consent()), None).await;
    assert_eq!(accepted.status(), StatusCode::OK);
}

#[tokio::test]
async fn account_email_rectification_requires_code_and_updates_session_identity() {
    let Some(ctx) = TestContext::start().await else {
        return;
    };
    let auth = complete_email_auth(&ctx, &unique_email("rectify")).await;
    let next_email = unique_email("rectified");

    let start = ctx
        .client
        .post(ctx.url("/account/email/start"))
        .header("X-Aegis-Request", "1")
        .header("Cookie", &auth.cookie)
        .json(&json!({ "email": next_email }))
        .send()
        .await
        .expect("start email update");
    assert_eq!(start.status(), StatusCode::OK);
    let body: Value = start.json().await.expect("start body");
    let challenge_id = body["challengeId"].as_str().expect("challenge id");
    let code = body["devCode"].as_str().expect("dev code");

    let verify = ctx
        .client
        .post(ctx.url("/account/email/verify"))
        .header("X-Aegis-Request", "1")
        .header("Cookie", &auth.cookie)
        .json(&json!({ "challengeId": challenge_id, "code": code }))
        .send()
        .await
        .expect("verify email update");
    assert_eq!(verify.status(), StatusCode::OK);
    let body: Value = verify.json().await.expect("verify body");
    assert_eq!(body["email"], next_email);

    let session = ctx
        .client
        .get(ctx.url("/auth/session"))
        .header("Cookie", &auth.cookie)
        .send()
        .await
        .expect("session after email update");
    assert_eq!(session.status(), StatusCode::OK);
    let body: Value = session.json().await.expect("session body");
    assert_eq!(body["user"]["email"], next_email);
}

#[tokio::test]
async fn account_delete_endpoint_refuses_when_balance_cannot_be_verified() {
    let Some(ctx) = TestContext::start().await else {
        return;
    };
    let auth = complete_email_auth(&ctx, &unique_email("delete-guard")).await;
    seed_real_wallet_routes(&ctx.pool, auth.user_id).await;

    let delete = ctx
        .client
        .post(ctx.url("/account/delete"))
        .header("X-Aegis-Request", "1")
        .header("Cookie", &auth.cookie)
        .json(&json!({ "confirm": true }))
        .send()
        .await
        .expect("delete account");
    assert_eq!(delete.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn tax_summary_lists_every_supported_wallet_route() {
    let Some(ctx) = TestContext::start().await else {
        return;
    };
    let auth = complete_email_auth(&ctx, &unique_email("tax-routes")).await;
    let portfolio_id = Uuid::new_v4();
    sqlx::query("INSERT INTO portfolios (id, user_id, name) VALUES ($1, $2, 'tax routes')")
        .bind(portfolio_id)
        .bind(auth.user_id)
        .execute(&ctx.pool)
        .await
        .expect("seed portfolio");
    seed_supported_wallet_routes(&ctx.pool, auth.user_id).await;

    let summary = ctx
        .client
        .get(ctx.url(&format!(
            "/tax/summary?portfolioId={portfolio_id}&year=2026"
        )))
        .header("Cookie", &auth.cookie)
        .send()
        .await
        .expect("tax summary");
    assert_eq!(summary.status(), StatusCode::OK);
    let body: Value = summary.json().await.expect("tax summary body");
    let chains: Vec<&str> = body["wallets"]
        .as_array()
        .expect("wallets")
        .iter()
        .map(|wallet| wallet["chain"].as_str().expect("chain"))
        .collect();
    assert_eq!(
        chains,
        vec![
            "ARC-TESTNET",
            "BASE-SEPOLIA",
            "ETH-SEPOLIA",
            "ARB-SEPOLIA",
            "AVAX-FUJI",
        ]
    );
}

#[tokio::test]
async fn wallet_reconciler_heals_pending_wallet_without_user_polling() {
    let Some(ctx) = TestContext::start().await else {
        return;
    };
    let user_id = seed_pending_user_without_wallet(&ctx.pool, &unique_email("reconcile")).await;
    let sse = sse::new_channel();
    let provider = MockProvider;
    let service = WalletService::new(&ctx.pool, &provider, &ctx.config, &sse);

    let healed = service
        .reconcile_pending_wallets(5000)
        .await
        .expect("reconcile pending wallets");
    assert!(healed >= 1);

    let status: String = sqlx::query_scalar("SELECT account_status FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&ctx.pool)
        .await
        .expect("account status");
    assert_eq!(status, "active");

    let routes: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM user_wallet_networks WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&ctx.pool)
            .await
            .expect("wallet route count");
    // A healed wallet is provisioned across every supported network, not just
    // the two execution rails — assert against the registry so this can't go
    // stale when the supported set changes.
    assert_eq!(
        routes,
        aegis_api::modules::wallet_routes::SUPPORTED_WALLET_BLOCKCHAINS.len() as i64
    );
}

struct TestContext {
    pool: PgPool,
    config: Config,
    client: reqwest::Client,
    client_key: String,
    addr: SocketAddr,
    _server: tokio::task::JoinHandle<()>,
}

impl Drop for TestContext {
    fn drop(&mut self) {
        self._server.abort();
    }
}

impl TestContext {
    async fn start() -> Option<Self> {
        let db_url =
            match std::env::var("TEST_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL")) {
                Ok(url) => url,
                Err(_) => {
                    eprintln!("DATABASE_URL not set; skipping auth_account_http integration test");
                    return None;
                }
            };
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&db_url)
            .await
            .expect("connect to test database");
        sqlx::migrate::Migrator::new(Path::new("./migrations"))
            .await
            .expect("load migrations")
            .run(&pool)
            .await
            .expect("run migrations");

        let config = test_config(&db_url);
        let app = router::build(pool.clone(), config.clone()).await;
        let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("test addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test app");
        });

        Some(Self {
            pool,
            config,
            client: reqwest::Client::new(),
            client_key: format!("test-{}", Uuid::new_v4()),
            addr,
            _server: server,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }
}

struct AuthResult {
    cookie: String,
    user_id: Uuid,
}

async fn complete_email_auth(ctx: &TestContext, email: &str) -> AuthResult {
    let (challenge_id, code) = start_code(ctx, email).await;
    let response = verify_code(ctx, &challenge_id, &code, Some(current_consent()), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = session_cookie(&response);
    let body: Value = response.json().await.expect("auth body");
    AuthResult {
        cookie,
        user_id: Uuid::parse_str(body["user"]["id"].as_str().expect("user id")).unwrap(),
    }
}

async fn start_code(ctx: &TestContext, email: &str) -> (String, String) {
    let response = start_code_response(ctx, email).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("start code body");
    let challenge_id = body["challengeId"]
        .as_str()
        .expect("challenge id")
        .to_string();
    let code = body["devCode"].as_str().expect("dev code").to_string();
    (challenge_id, code)
}

async fn start_code_response(ctx: &TestContext, email: &str) -> reqwest::Response {
    ctx.client
        .post(ctx.url("/auth/email/start"))
        .header("X-Aegis-Request", "1")
        .header("X-Forwarded-For", &ctx.client_key)
        .json(&json!({ "email": email }))
        .send()
        .await
        .expect("start code")
}

async fn verify_code(
    ctx: &TestContext,
    challenge_id: &str,
    code: &str,
    consent: Option<Value>,
    cookie: Option<&str>,
) -> reqwest::Response {
    let mut body = json!({ "challengeId": challenge_id, "code": code });
    if let Some(consent) = consent {
        body["consent"] = consent;
    }
    let mut req = ctx
        .client
        .post(ctx.url("/auth/email/verify"))
        .header("X-Aegis-Request", "1")
        .header("X-Forwarded-For", &ctx.client_key)
        .json(&body);
    if let Some(cookie) = cookie {
        req = req.header("Cookie", cookie);
    }
    req.send().await.expect("verify code")
}

fn current_consent() -> Value {
    json!({
        "tos": true,
        "privacy": true,
        "tosVersion": "2026-05",
        "privacyVersion": "2026-05",
        "marketingOptIn": false
    })
}

fn session_cookie(response: &reqwest::Response) -> String {
    response
        .headers()
        .get("set-cookie")
        .expect("set-cookie")
        .to_str()
        .expect("cookie string")
        .split(';')
        .next()
        .expect("cookie pair")
        .to_string()
}

fn session_id_from_cookie(cookie: &str) -> Uuid {
    let (_, value) = cookie.split_once('=').expect("cookie pair");
    Uuid::parse_str(value).expect("session uuid")
}

async fn error_code(response: reqwest::Response) -> String {
    let body: Value = response.json().await.expect("error body");
    body["error"]["code"].as_str().expect("error code").into()
}

fn strip_challenge_specific_fields(body: &mut Value) {
    if let Some(obj) = body.as_object_mut() {
        obj.remove("challengeId");
        obj.remove("email");
        obj.remove("expiresAt");
        obj.remove("devCode");
    }
}

async fn seed_user(pool: &PgPool, email: &str, tos_version: &str, privacy_version: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users (
            email, account_status, custody_model,
            tos_version, privacy_version, consented_at
         )
         VALUES ($1, 'active', 'circle_developer', $2, $3, NOW())
         RETURNING id",
    )
    .bind(email)
    .bind(tos_version)
    .bind(privacy_version)
    .fetch_one(pool)
    .await
    .expect("seed user")
}

async fn seed_pending_user_without_wallet(pool: &PgPool, email: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users (
            email, account_status, custody_model,
            tos_version, privacy_version, consented_at,
            wallet_provision_next_retry_at
         )
         VALUES ($1, 'pending_wallet', 'circle_developer', '2026-05', '2026-05', NOW(), NOW())
         RETURNING id",
    )
    .bind(email)
    .fetch_one(pool)
    .await
    .expect("seed pending user")
}

async fn seed_real_wallet_routes(pool: &PgPool, user_id: Uuid) {
    for (blockchain, wallet_id, address) in [
        (
            "ARC-TESTNET",
            "wallet-arc-real",
            "0x1111111111111111111111111111111111111111",
        ),
        (
            "BASE-SEPOLIA",
            "wallet-base-real",
            "0x2222222222222222222222222222222222222222",
        ),
    ] {
        sqlx::query(
            "INSERT INTO user_wallet_networks (
                user_id, blockchain, circle_wallet_id, address,
                account_type, wallet_set_id, state
             )
             VALUES ($1, $2, $3, $4, 'SCA', 'test-wallet-set', 'LIVE')
             ON CONFLICT (user_id, blockchain) DO UPDATE
                SET circle_wallet_id = EXCLUDED.circle_wallet_id,
                    address = EXCLUDED.address,
                    wallet_set_id = EXCLUDED.wallet_set_id,
                    state = EXCLUDED.state",
        )
        .bind(user_id)
        .bind(blockchain)
        .bind(wallet_id)
        .bind(address)
        .execute(pool)
        .await
        .expect("seed route");
    }
}

async fn seed_supported_wallet_routes(pool: &PgPool, user_id: Uuid) {
    for (blockchain, wallet_id, address) in [
        (
            "ARC-TESTNET",
            "wallet-arc-real",
            "0x1111111111111111111111111111111111111111",
        ),
        (
            "BASE-SEPOLIA",
            "wallet-base-real",
            "0x2222222222222222222222222222222222222222",
        ),
        (
            "ETH-SEPOLIA",
            "wallet-eth-real",
            "0x3333333333333333333333333333333333333333",
        ),
        (
            "ARB-SEPOLIA",
            "wallet-arb-real",
            "0x4444444444444444444444444444444444444444",
        ),
        (
            "AVAX-FUJI",
            "wallet-avax-real",
            "0x5555555555555555555555555555555555555555",
        ),
    ] {
        sqlx::query(
            "INSERT INTO user_wallet_networks (
                user_id, blockchain, circle_wallet_id, address,
                account_type, wallet_set_id, state
             )
             VALUES ($1, $2, $3, $4, 'SCA', 'test-wallet-set', 'LIVE')
             ON CONFLICT (user_id, blockchain) DO UPDATE
                SET circle_wallet_id = EXCLUDED.circle_wallet_id,
                    address = EXCLUDED.address,
                    wallet_set_id = EXCLUDED.wallet_set_id,
                    state = EXCLUDED.state",
        )
        .bind(user_id)
        .bind(blockchain)
        .bind(wallet_id)
        .bind(address)
        .execute(pool)
        .await
        .expect("seed supported route");
    }
}

fn unique_email(prefix: &str) -> String {
    format!("{prefix}-{}@example.com", Uuid::new_v4())
}

fn test_config(database_url: &str) -> Config {
    Config {
        database_url: database_url.into(),
        jwt_secret: "test-secret".into(),
        jwt_expiry_hours: 24,
        session_idle_timeout_minutes: 30,
        host: "127.0.0.1".into(),
        port: 0,
        openrouter_api_key: "test".into(),
        openrouter_base_url: "https://openrouter.ai/api/v1".into(),
        model_regime: "regime-model".into(),
        model_strategist: "strategist-model".into(),
        model_critic: "critic-model".into(),
        model_tax: "tax-model".into(),
        model_commentary: "commentary-model".into(),
        openrouter_app_name: "Aegis".into(),
        openrouter_app_url: None,
        coingecko_api_key: None,
        price_provider_primary: "defillama".into(),
        price_provider_fallback: "none".into(),
        sse_price_tick_secs: 3600,
        circle_api_key: "circle-key".into(),
        circle_base_url: "https://api.circle.com".into(),
        circle_env: "sandbox".into(),
        circle_wallet_set_id: "test-wallet-set".into(),
        circle_entity_secret: "0000000000000000000000000000000000000000000000000000000000000000"
            .into(),
        circle_mock: true,
        arc_rpc_url: "https://testnet.arc.network".into(),
        base_rpc_url: "https://sepolia.base.org".into(),
        eth_rpc_url: String::new(),
        arb_rpc_url: String::new(),
        avax_rpc_url: String::new(),
        op_rpc_url: String::new(),
        gateway_poll_secs: 3600,
        faucet_max_usdc_per_day: 100.0,
        cors_allow_origin: "http://localhost:3000".into(),
        session_cookie_name: "aegis_session".into(),
        session_cookie_secure: false,
        cctp_attestation_url: "https://iris-api-sandbox.circle.com".into(),
        cctp_attestation_timeout_secs: 180,
        chain_private_key_arc: String::new(),
        chain_private_key_base: String::new(),
        chain_private_key_eth: String::new(),
        chain_private_key_arb: String::new(),
        chain_private_key_avax: String::new(),
        chain_private_key_op: String::new(),
        cctp_token_messenger_arc: String::new(),
        cctp_token_messenger_base: String::new(),
        cctp_token_messenger_eth: String::new(),
        cctp_token_messenger_arb: String::new(),
        cctp_token_messenger_avax: String::new(),
        cctp_token_messenger_op: String::new(),
        cctp_message_transmitter_arc: String::new(),
        cctp_message_transmitter_base: String::new(),
        cctp_message_transmitter_eth: String::new(),
        cctp_message_transmitter_arb: String::new(),
        cctp_message_transmitter_avax: String::new(),
        cctp_message_transmitter_op: String::new(),
        rebalance_executor_arc: String::new(),
        rebalance_executor_base: String::new(),
        usdc_arc: String::new(),
        usdc_base: String::new(),
        usdc_eth: String::new(),
        usdc_arb: String::new(),
        usdc_avax: String::new(),
        usdc_op: String::new(),
        usyc_token_arc: String::new(),
        usyc_teller_arc: String::new(),
        usyc_oracle_arc: String::new(),
        usyc_enabled: false,
        uniswap_v3_quoter_base: String::new(),
        uniswap_v3_router_base: String::new(),
        weth_base: String::new(),
        cbbtc_base: String::new(),
        cbeth_base: String::new(),
        susds_base: String::new(),
        uniswap_v3_quoter_eth: String::new(),
        uniswap_v3_router_eth: String::new(),
        uniswap_v3_quoter_arb: String::new(),
        uniswap_v3_router_arb: String::new(),
        uniswap_v3_quoter_op: String::new(),
        uniswap_v3_router_op: String::new(),
        weth_eth: String::new(),
        weth_arb: String::new(),
        weth_op: String::new(),
        wbtc_eth: String::new(),
        wbtc_arb: String::new(),
        nanopayments_facilitator_url: "https://gateway-api-testnet.circle.com".into(),
        nanopayments_seller_address: String::new(),
        nanopayments_treasury_address: String::new(),
        billing_v2_enabled: false,
        admin_user_ids: vec![],
        execution_mock: true,
        circle_wallet_exec: false,
        scheduler_tick_secs: 3600,
        scheduler_cooldown_secs: 1800,
        harvest_threshold_usd: 50.0,
        openrouter_budget_guard_usd: 0.05,
        stablefx_institutional_access: false,
        digest_hour_utc: 8,
        resend_api_key: String::new(),
        digest_from: "Aegis <noreply@aegis.local>".into(),
        digest_secret: "test-secret".into(),
        public_base_url: "http://localhost:3000".into(),
        api_base_url: "http://localhost:8080".into(),
        regime_backtest_enabled: true,
        peg_defense_enabled: true,
        peg_monitor_tick_secs: 3600,
        peg_fire_cooldown_secs: 1800,
        tax_export_v1_enabled: true,
        aum_stream_enabled: false,
        calibrated_conf_enabled: false,
        constitution_enabled: false,
    }
}
