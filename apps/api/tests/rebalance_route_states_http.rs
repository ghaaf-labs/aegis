//! Integration coverage for the execution route registry and the
//! real-execution metrics contract.
//!
//! Two layers:
//!   1. Route-state matrix — drives the *public* `route::validate_legs` with a
//!      real-mode `Config` and asserts each route fails closed with the right
//!      blocker (no DB needed; always runs).
//!   2. Metrics SQL contract — against a real Postgres (`TEST_DATABASE_URL`),
//!      proves migration 0033's `v_trustability_per_user` counts only real,
//!      completed executions. Skipped when `TEST_DATABASE_URL` is unset so
//!      `cargo test --all-targets` stays hermetic.

use aegis_api::config::{ChainConfig, Config};
use aegis_api::modules::rebalance::registry::{
    route::RouteLeg, validate_legs, BlockerCode, RuntimeCapabilities,
};
use aegis_api::modules::rebalance::ChainKey;

/// A real-mode config (neither execution nor Circle mocked) with signers set so
/// `RuntimeCapabilities::real_mode` is true and only feature/adapter gaps remain.
fn real_config() -> Config {
    let mut cfg = base_config();
    cfg.execution_mock = false;
    cfg.circle_mock = false;
    cfg.chains[ChainKey::Arc.index()].private_key =
        "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into();
    cfg.chains[ChainKey::Base.index()].private_key =
        "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into();
    cfg.chains[ChainKey::Arc.index()].usdc = "0x00000000000000000000000000000000000000a1".into();
    cfg.chains[ChainKey::Base.index()].usdc = "0x036CbD53842c5426634e7929541eC2318f3dCF7e".into();
    cfg
}

fn leg(kind: &str, src: &str, dest: &str, src_sym: &str, dest_sym: &str) -> RouteLeg {
    RouteLeg::from_parts(
        kind,
        Some(src.into()),
        Some(dest.into()),
        Some(src_sym.into()),
        Some(dest_sym.into()),
        40.0,
    )
    .expect("known leg kind")
}

fn has(
    blockers: &[aegis_api::modules::rebalance::registry::RouteBlocker],
    code: BlockerCode,
) -> bool {
    blockers.iter().any(|b| b.code == code)
}

#[test]
fn usyc_park_fails_closed_in_real_mode() {
    let cfg = real_config();
    let caps = RuntimeCapabilities::from_config(&cfg);
    let blockers = validate_legs(
        &caps,
        &cfg,
        &[leg("park_usyc", "arc", "arc", "USDC", "USYC")],
    );
    assert!(has(&blockers, BlockerCode::UsycDisabled));
}

#[test]
fn stablefx_fails_closed_in_real_mode() {
    let cfg = real_config();
    let caps = RuntimeCapabilities::from_config(&cfg);
    let blockers = validate_legs(
        &caps,
        &cfg,
        &[leg("fx_stablefx", "arc", "arc", "USDC", "EURC")],
    );
    assert!(has(&blockers, BlockerCode::StablefxUnavailable));
}

#[test]
fn non_execution_chain_is_blocked() {
    // A chain with no provisioned wallet route / non-EVM chain fails closed with
    // a NonExecutionChain blocker. (ETH/Arb/Avax are now execution chains for
    // the CCTP consolidation baseline — see `unconfigured_cctp_source_chain_is_blocked`
    // for how an unwired-but-executable chain still fails closed per-leg.)
    let cfg = real_config();
    let caps = RuntimeCapabilities::from_config(&cfg);
    for chain in ["solana", "op-sepolia"] {
        let blockers = validate_legs(
            &caps,
            &cfg,
            &[leg("cross_chain_burn", chain, "base", "USDC", "USDC")],
        );
        assert!(
            has(&blockers, BlockerCode::NonExecutionChain),
            "{chain} must be blocked as a non-execution chain"
        );
    }
}

#[cfg(feature = "real-cctp")]
#[test]
fn unconfigured_cctp_source_chain_is_blocked() {
    // ETH-Sepolia is an execution chain, but `real_config()` only wires Arc/Base
    // USDC + signers. A burn sourced from ETH-Sepolia must still fail closed —
    // per-leg CCTP validation sees the missing ETH USDC/messenger and blocks it
    // at approval rather than after the source burn has left the wallet.
    let cfg = real_config();
    let caps = RuntimeCapabilities::from_config(&cfg);
    let blockers = validate_legs(
        &caps,
        &cfg,
        &[leg(
            "cross_chain_burn",
            "eth-sepolia",
            "base",
            "USDC",
            "USDC",
        )],
    );
    assert!(
        has(&blockers, BlockerCode::UsdcAddress),
        "an unconfigured CCTP source chain must fail closed, got {blockers:?}"
    );
}

#[cfg(not(feature = "real-cctp"))]
#[test]
fn usdc_bridge_needs_real_cctp_feature() {
    let cfg = real_config();
    let caps = RuntimeCapabilities::from_config(&cfg);
    let blockers = validate_legs(
        &caps,
        &cfg,
        &[leg("cross_chain_burn", "arc", "base", "USDC", "USDC")],
    );
    assert!(has(&blockers, BlockerCode::RealCctpFeature));
}

#[cfg(not(feature = "real-swap"))]
#[test]
fn local_swap_needs_real_swap_feature() {
    let cfg = real_config();
    let caps = RuntimeCapabilities::from_config(&cfg);
    let blockers = validate_legs(
        &caps,
        &cfg,
        &[leg("local_swap", "base", "base", "USDC", "ETH")],
    );
    assert!(has(&blockers, BlockerCode::RealSwapFeature));
}

#[cfg(not(feature = "real-swap"))]
#[test]
fn eurc_routes_as_base_local_swap_not_stablefx() {
    // The EUR sleeve now executes through the Base USDC/EURC DEX pool, so a
    // USDC→EURC buy is a local_swap on Base — it must clear the swap rail (here
    // just the feature gate), never the gated StableFX blocker.
    let mut cfg = real_config();
    cfg.swap_liquid_tokens
        .insert(ChainKey::Base, vec!["EURC".into()]);
    let caps = RuntimeCapabilities::from_config(&cfg);
    let blockers = validate_legs(
        &caps,
        &cfg,
        &[leg("local_swap", "base", "base", "USDC", "EURC")],
    );
    assert!(has(&blockers, BlockerCode::RealSwapFeature));
    assert!(!has(&blockers, BlockerCode::StablefxUnavailable));
}

#[cfg(feature = "real-swap")]
#[test]
fn eurc_base_swap_is_executable_when_configured() {
    // With the swap venue + EURC's Base ERC-20 configured (and the real-swap
    // feature compiled in), a USDC→EURC local_swap on Base has no blockers.
    let mut cfg = real_config();
    cfg.chains[ChainKey::Base.index()].swap_quoter =
        "0xC5290058841028F1614F3A6F0F5816cAd0df5E27".into();
    cfg.chains[ChainKey::Base.index()].swap_router =
        "0x94cC0AaC535CCDB3C01d6787D6413C739ae12bc4".into();
    cfg.set_token_address(
        "EURC",
        ChainKey::Base,
        "0x60a3E35Cc302bFA44Cb288Bc5a4F316Fdb1adb42",
    );
    cfg.swap_liquid_tokens
        .insert(ChainKey::Base, vec!["EURC".into()]);
    let caps = RuntimeCapabilities::from_config(&cfg);
    let blockers = validate_legs(
        &caps,
        &cfg,
        &[leg("local_swap", "base", "base", "USDC", "EURC")],
    );
    assert!(
        blockers.is_empty(),
        "configured EURC Base swap must be executable, got {blockers:?}"
    );
}

#[cfg(feature = "real-swap")]
#[test]
fn eurc_base_swap_fails_closed_without_address() {
    // Swap venue live but EURC's Base ERC-20 unset → fail closed on the address.
    let mut cfg = real_config();
    cfg.chains[ChainKey::Base.index()].swap_quoter =
        "0xC5290058841028F1614F3A6F0F5816cAd0df5E27".into();
    cfg.chains[ChainKey::Base.index()].swap_router =
        "0x94cC0AaC535CCDB3C01d6787D6413C739ae12bc4".into();
    let caps = RuntimeCapabilities::from_config(&cfg);
    let blockers = validate_legs(
        &caps,
        &cfg,
        &[leg("local_swap", "base", "base", "USDC", "EURC")],
    );
    assert!(has(&blockers, BlockerCode::SwapTokenAddress));
}

#[test]
fn mock_mode_permits_every_leg() {
    let cfg = base_config(); // execution_mock = true
    let caps = RuntimeCapabilities::from_config(&cfg);
    let legs = vec![
        leg("park_usyc", "arc", "arc", "USDC", "USYC"),
        leg("fx_stablefx", "arc", "arc", "USDC", "EURC"),
        leg("local_swap", "base", "base", "USDC", "ETH"),
    ];
    assert!(validate_legs(&caps, &cfg, &legs).is_empty());
}

// ── Metrics SQL contract (needs TEST_DATABASE_URL) ─────────────────────────

#[tokio::test]
async fn trustability_view_counts_only_real_completed_executions() {
    use sqlx::postgres::PgPoolOptions;
    use std::path::Path as FsPath;
    use uuid::Uuid;

    let Ok(db_url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("TEST_DATABASE_URL not set; skipping trustability metrics integration test");
        return;
    };

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .expect("connect to TEST_DATABASE_URL");
    sqlx::migrate::Migrator::new(FsPath::new("./migrations"))
        .await
        .expect("load migrations")
        .run(&pool)
        .await
        .expect("run migrations");

    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, arc_address)
         VALUES ($1, '0xdeadbeef00000000000000000000000000000077') RETURNING id",
    )
    .bind(format!("route-metrics-{}@example.com", Uuid::new_v4()))
    .fetch_one(&pool)
    .await
    .expect("insert user");

    let portfolio_id: Uuid = sqlx::query_scalar(
        "INSERT INTO portfolios (user_id, name) VALUES ($1, 'route-metrics') RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("insert portfolio");

    // Two decisions: one led to a MOCK completed rebalance, one to a REAL one.
    let mock_decision: Uuid = sqlx::query_scalar(
        "INSERT INTO agent_decisions (portfolio_id, reasoning, triggered_by, model_slug)
         VALUES ($1, 'mock', 'user_request', 'model/a') RETURNING id",
    )
    .bind(portfolio_id)
    .fetch_one(&pool)
    .await
    .expect("insert mock decision");
    let real_decision: Uuid = sqlx::query_scalar(
        "INSERT INTO agent_decisions (portfolio_id, reasoning, triggered_by, model_slug)
         VALUES ($1, 'real', 'user_request', 'model/b') RETURNING id",
    )
    .bind(portfolio_id)
    .fetch_one(&pool)
    .await
    .expect("insert real decision");

    for (decision, mode) in [(mock_decision, "mock"), (real_decision, "real")] {
        sqlx::query(
            "INSERT INTO rebalances (portfolio_id, decision_id, status, total_legs, execution_mode)
             VALUES ($1, $2, 'completed', 1, $3)",
        )
        .bind(portfolio_id)
        .bind(decision)
        .bind(mode)
        .execute(&pool)
        .await
        .expect("insert rebalance");
    }

    // Give the real decision a 24h outcome so the view's averages populate.
    sqlx::query(
        "INSERT INTO agent_memory (portfolio_id, decision_id, outcome_24h)
         VALUES ($1, $2, '{\"realizedPctChange\": 1.5, \"counterfactualPctChange\": 0.5}'::jsonb)",
    )
    .bind(portfolio_id)
    .bind(real_decision)
    .execute(&pool)
    .await
    .expect("insert agent_memory");

    let decisions_executed: i64 = sqlx::query_scalar(
        "SELECT decisions_executed FROM v_trustability_per_user WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("query trustability view");

    assert_eq!(
        decisions_executed, 1,
        "only the real, completed rebalance's decision should count toward public metrics"
    );
}

/// Minimal mock-mode config. Built field-by-field because the crate's
/// `test_config` helper is `#[cfg(test)]`-private to the lib and not visible
/// from this integration crate.
fn base_config() -> Config {
    Config {
        database_url: "postgres://test".into(),
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
        openrouter_max_retries: 1,
        openrouter_attempt_timeout_secs: 90,
        openrouter_response_healing: true,
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
        chains: [
            ChainConfig {
                rpc_url: "https://testnet.arc.network".into(),
                ..ChainConfig::default()
            },
            ChainConfig {
                rpc_url: "https://sepolia.base.org".into(),
                ..ChainConfig::default()
            },
            ChainConfig::default(),
            ChainConfig::default(),
            ChainConfig::default(),
            ChainConfig::default(),
        ],
        gateway_poll_secs: 3600,
        faucet_max_usdc_per_day: 100.0,
        cors_allow_origin: "http://localhost:3000".into(),
        session_cookie_name: "aegis_session".into(),
        session_cookie_secure: false,
        cctp_attestation_url: "https://iris-api-sandbox.circle.com".into(),
        cctp_attestation_timeout_secs: 180,
        usyc_token_arc: String::new(),
        usyc_teller_arc: String::new(),
        usyc_oracle_arc: String::new(),
        usyc_enabled: false,
        token_addrs: std::collections::HashMap::new(),
        swap_liquid_tokens: std::collections::HashMap::new(),
        swap_pool_depth_usd: std::collections::HashMap::new(),
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
