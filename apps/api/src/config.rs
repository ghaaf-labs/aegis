use anyhow::Context;

use crate::modules::rebalance::models::ChainKey;

/// Per-task model resolution: every AI call site declares its `ModelRoute`,
/// and `Config::model_for(route)` returns the slug. Slugs are env-driven so
/// switching providers requires zero code changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelRoute {
    RegimeClassify,
    RebalanceReason,
    /// Used by the tax-loss explainer (Sprint 3). Variant present so the
    /// agent service compiles against the full API today.
    #[allow(dead_code)]
    TaxExplain,
    /// Used by the daily commentary generator (Sprint 2).
    #[allow(dead_code)]
    MarketCommentary,
    CritiqueAgent,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    /// Server-side HMAC secret used for email-code hashes and other internal
    /// auth tokens. Browser sessions are opaque IDs, not JWTs.
    pub jwt_secret: String,
    /// Absolute session lifetime in hours.
    pub jwt_expiry_hours: u64,
    /// Idle session lifetime in minutes. Any session quiet longer than this is
    /// rejected even if its absolute TTL has not elapsed.
    pub session_idle_timeout_minutes: u64,
    pub host: String,
    pub port: u16,

    // ── OpenRouter ─────────────────────────────────────────────────────────
    pub openrouter_api_key: String,
    pub openrouter_base_url: String,

    // Per-route model slugs. Defaults below; override via env.
    pub model_regime: String,
    pub model_strategist: String,
    pub model_critic: String,
    pub model_tax: String,
    pub model_commentary: String,

    // Optional referer / app name OpenRouter records for analytics.
    pub openrouter_app_name: String,
    pub openrouter_app_url: Option<String>,

    pub coingecko_api_key: Option<String>,

    /// Price provider used by market_data, peg_monitor and fx. Selectable so
    /// a runtime flip in `.env.local` rolls back to CoinGecko if a new provider
    /// misbehaves. Accepted values: "defillama" | "pyth" | "coingecko".
    pub price_provider_primary: String,
    /// Fallback provider used by the in-process circuit breaker once the
    /// primary trips its failure threshold. "none" disables fallback.
    pub price_provider_fallback: String,

    /// Cadence (seconds) for the SSE price ticker. Lower = more "realtime"
    /// feel; higher = friendlier to upstream rate limits.
    pub sse_price_tick_secs: u64,

    // ── Circle (Wallets, Gateway, Paymaster, USYC, StableFX) ──────────────
    pub circle_api_key: String,
    pub circle_base_url: String,
    #[allow(dead_code)]
    pub circle_env: String,
    /// Wallet set used for server-side Circle developer-controlled wallets.
    /// Empty leaves new accounts in `pending_wallet` until configured.
    pub circle_wallet_set_id: String,
    /// Hex-encoded Circle entity secret. Used only server-side to generate a
    /// fresh entitySecretCiphertext for each developer-controlled wallet call.
    pub circle_entity_secret: String,
    /// When true, the wallet module uses an in-process mock provider instead
    /// of hitting Circle WaaS. Defaults to `false` (real-by-default); set
    /// `MOCK_CIRCLE=true` in `.env.local` for offline dev or hermetic CI.
    pub circle_mock: bool,

    #[allow(dead_code)]
    pub arc_rpc_url: String,
    #[allow(dead_code)]
    pub base_rpc_url: String,
    #[allow(dead_code)]
    pub eth_rpc_url: String,
    #[allow(dead_code)]
    pub arb_rpc_url: String,
    #[allow(dead_code)]
    pub avax_rpc_url: String,
    #[allow(dead_code)]
    pub op_rpc_url: String,

    /// Cadence for the Gateway unified-balance ticker. Read by S2.6.
    #[allow(dead_code)]
    pub gateway_poll_secs: u64,

    /// 24h USDC faucet rate limit per wallet. Read by S2.4.
    #[allow(dead_code)]
    pub faucet_max_usdc_per_day: f64,

    /// Comma-separated list of allowed CORS origins. HttpOnly-cookie auth
    /// requires a specific origin (browsers reject `*` with credentials),
    /// so production deploys must set this.
    pub cors_allow_origin: String,

    /// Cookie name for the opaque session id.
    pub session_cookie_name: String,

    /// When true, the `Secure` flag is set on the auth cookie. Default true
    /// in production; flip to false for `http://localhost` dev.
    pub session_cookie_secure: bool,

    // ── Sprint 3: cross-chain execution ───────────────────────────────────
    /// Circle CCTP V2 attestation API base URL.
    #[allow(dead_code)]
    pub cctp_attestation_url: String,
    /// Max wall-clock seconds we'll wait for a CCTP attestation. Default 180.
    #[allow(dead_code)]
    pub cctp_attestation_timeout_secs: u64,
    /// EOA private key (hex, 0x-prefixed) used to submit transactions on Arc.
    /// Empty in mock mode.
    #[allow(dead_code)]
    pub chain_private_key_arc: String,
    /// EOA private key for Base Sepolia transactions.
    #[allow(dead_code)]
    pub chain_private_key_base: String,
    /// EOA private keys for the additional CCTP-V2 testnets. Empty until the
    /// chain is wired for execution (it stays non-execution while blank).
    #[allow(dead_code)]
    pub chain_private_key_eth: String,
    #[allow(dead_code)]
    pub chain_private_key_arb: String,
    #[allow(dead_code)]
    pub chain_private_key_avax: String,
    #[allow(dead_code)]
    pub chain_private_key_op: String,

    // Real CCTP + Hook execution addresses (loaded only when real-cctp feature + !mock)
    #[allow(dead_code)]
    pub cctp_token_messenger_arc: String,
    #[allow(dead_code)]
    pub cctp_token_messenger_base: String,
    #[allow(dead_code)]
    pub cctp_token_messenger_eth: String,
    #[allow(dead_code)]
    pub cctp_token_messenger_arb: String,
    #[allow(dead_code)]
    pub cctp_token_messenger_avax: String,
    #[allow(dead_code)]
    pub cctp_token_messenger_op: String,
    #[allow(dead_code)]
    pub cctp_message_transmitter_arc: String,
    #[allow(dead_code)]
    pub cctp_message_transmitter_base: String,
    #[allow(dead_code)]
    pub cctp_message_transmitter_eth: String,
    #[allow(dead_code)]
    pub cctp_message_transmitter_arb: String,
    #[allow(dead_code)]
    pub cctp_message_transmitter_avax: String,
    #[allow(dead_code)]
    pub cctp_message_transmitter_op: String,
    #[allow(dead_code)]
    pub rebalance_executor_arc: String,
    #[allow(dead_code)]
    pub rebalance_executor_base: String,
    #[allow(dead_code)]
    pub usdc_arc: String,
    #[allow(dead_code)]
    pub usdc_base: String,
    #[allow(dead_code)]
    pub usdc_eth: String,
    #[allow(dead_code)]
    pub usdc_arb: String,
    #[allow(dead_code)]
    pub usdc_avax: String,
    #[allow(dead_code)]
    pub usdc_op: String,
    #[allow(dead_code)]
    pub usyc_token_arc: String,
    #[allow(dead_code)]
    pub usyc_teller_arc: String,
    #[allow(dead_code)]
    pub usyc_oracle_arc: String,
    /// Kill-switch for the USYC park/redeem sleeve. Default `false`: the
    /// Hashnote Teller on Arc testnet is allowlist/KYB-gated (deposits revert
    /// `0x7f63bd0f`), and Circle's CCTP docs list USYC only on Ethereum/BNB.
    /// While `false` the route registry marks USYC non-executable (Track-only)
    /// so it can never be approved or executed. Flip to `true` once the Aegis
    /// EOA is allowlisted and the real Teller path is wired.
    pub usyc_enabled: bool,

    // ── Per-chain swap venue (Uniswap V3, Base Sepolia) ────────────────────
    /// Uniswap V3 QuoterV2 used to price USDC↔token swaps and derive `min_out`.
    /// Empty ⇒ the swap adapter reports `NeedsAddress` and swaps fail closed.
    #[allow(dead_code)]
    pub uniswap_v3_quoter_base: String,
    /// Uniswap V3 SwapRouter02 used to execute USDC↔token swaps on Base.
    #[allow(dead_code)]
    pub uniswap_v3_router_base: String,
    /// Wrapped-ETH ERC-20 on Base Sepolia — the concrete token behind the
    /// "ETH" symbol for swap routing. Other volatiles have no canonical Base
    /// Sepolia ERC-20 + pool, so they fail closed (`NeedsAddress`).
    #[allow(dead_code)]
    pub weth_base: String,
    /// Coinbase Wrapped BTC ERC-20 on Base (BTC sleeve). Empty ⇒ track-only.
    #[allow(dead_code)]
    pub cbbtc_base: String,
    /// Coinbase Wrapped Staked ETH ERC-20 on Base (staked-ETH yield sleeve).
    #[allow(dead_code)]
    pub cbeth_base: String,
    /// Sky sUSDS ERC-20 on Base (permissionless savings-yield sleeve).
    #[allow(dead_code)]
    pub susds_base: String,

    // ── Nanopayments (x402) for 25bps protocol fee + referrals ────────────
    #[allow(dead_code)]
    pub nanopayments_facilitator_url: String,
    #[allow(dead_code)]
    pub nanopayments_seller_address: String,
    /// Treasury wallet address that pays out referral rewards via
    /// Nanopayments. Required when `BILLING_V2_ENABLED=true`; validated at
    /// startup so the API never boots with this flag on but the wallet
    /// unset. Empty when the flag is off.
    #[allow(dead_code)]
    pub nanopayments_treasury_address: String,
    /// Master feature flag for the post-hackathon real-revenue path:
    /// real Nanopayments settlement, refunds, real referral payouts, AUM
    /// streaming, subscriptions, tier gating. Defaults `false` so `main`
    /// stays trunk-shippable.
    #[allow(dead_code)]
    pub billing_v2_enabled: bool,

    /// Comma-separated list of user UUIDs that may call `/admin/*` endpoints.
    /// Empty (default) disables all admin routes regardless of feature flags.
    pub admin_user_ids: Vec<uuid::Uuid>,

    /// When true, the executor / cross-chain client skip real RPC calls and
    /// use mock adapters. Defaults to `false` (real-by-default); set
    /// `EXECUTION_MOCK=true` (the opt-in test/CI/offline path) to mock execution.
    pub execution_mock: bool,

    /// Part B0 — non-custodial execution. When true, real on-chain legs
    /// (CCTP burn/mint, USDC approve, per-chain swap) are submitted from the
    /// *user's* Circle developer-controlled wallet via Circle's
    /// Create-Contract-Execution-Transaction API (entity-secret signed) rather
    /// than from a backend EOA. Default `false`: the EOA path (`CHAIN_PRIVATE_KEY_*`)
    /// stays the verified default so nothing regresses. Flip to `true` only with
    /// live Circle developer creds + a funded user wallet — the round-trip is
    /// untestable offline (same caveat as the existing real CCTP/swap paths).
    pub circle_wallet_exec: bool,

    // ── Sprint 3: scheduler ───────────────────────────────────────────────
    /// Tick cadence (seconds) for the per-portfolio drift watcher.
    pub scheduler_tick_secs: u64,
    /// Per-portfolio cooldown (seconds) — no decision emitted within this
    /// window after one already landed.
    pub scheduler_cooldown_secs: u64,
    /// Harvestable-losses threshold (USD). Below this, no harvest signal.
    pub harvest_threshold_usd: f64,

    /// F-COST-1: soft budget guard. When an OpenRouter call's reported
    /// cost exceeds this number of USD per decision, the agent service
    /// logs a `warn!` and (once the enforcement path lands) routes the
    /// next call to the cheaper Haiku tier. Default $0.05/decision —
    /// roughly the cost ceiling of a typical strategist + critic pass
    /// at v4-flash + Haiku 4.5 mid-2026 pricing.
    /// Enforced at call time in `modules/ai/client.rs::check_budget_guard`
    /// (HS-2 / F-COST-2 closed 2026-05-17): structured warn fires when
    /// per-call cost exceeds this number.
    #[allow(dead_code)]
    pub openrouter_budget_guard_usd: f64,
    /// HS-6 — when true, the FX module attempts a Circle StableFX RFQ
    /// before falling back to CoinGecko spot. Default `false`; institutional
    /// access is KYB-gated and not yet open to retail Aegis. Keeping the
    /// env in place so flipping it later is a 1-line config change.
    #[allow(dead_code)]
    pub stablefx_institutional_access: bool,

    // ── Sprint 3: digest ──────────────────────────────────────────────────
    /// Hour of day (UTC) when the digest worker fires. 0–23.
    #[allow(dead_code)]
    pub digest_hour_utc: u32,
    /// Resend API key. Empty disables email sends.
    #[allow(dead_code)]
    pub resend_api_key: String,
    /// Sender address (must be verified in Resend).
    #[allow(dead_code)]
    pub digest_from: String,
    /// HMAC secret for digest unsubscribe tokens.
    pub digest_secret: String,
    /// Base URL used when building unsubscribe links. Usually the public
    /// frontend URL.
    #[allow(dead_code)]
    pub public_base_url: String,
    /// Base URL of this API service — used in emails so unsubscribe links
    /// resolve to the backend route, not the frontend.
    pub api_base_url: String,

    /// F-REG-6: gates the regime-backtest admin endpoints (kick off a run,
    /// list latest evaluations). The public `/about/regime` model card reads
    /// already-persisted rows directly from Postgres and is *not* gated, so
    /// the model card keeps working even with the flag off.
    pub regime_backtest_enabled: bool,

    // ── Peg-defense (A6) ──────────────────────────────────────────────────
    #[allow(dead_code)]
    pub peg_defense_enabled: bool,
    #[allow(dead_code)]
    pub peg_monitor_tick_secs: u64,
    #[allow(dead_code)]
    pub peg_fire_cooldown_secs: i64,

    /// A10 — 1099-DA tax export. Default false.
    #[allow(dead_code)]
    pub tax_export_v1_enabled: bool,

    /// A4 — AUM-fee accrual ticker. Requires `billing_v2_enabled=true`.
    #[allow(dead_code)]
    pub aum_stream_enabled: bool,

    /// F-CONF-7: gates calibration trainer + per-decision calibration apply
    /// + critic counterfactual. Default false.
    #[allow(dead_code)]
    pub calibrated_conf_enabled: bool,

    /// F-CON-6 (A9): when true, the critic runs the Aegis Constitution check
    /// first and short-circuits to VETO on any clause violation. When false,
    /// the YAML still loads but `evaluate()` is bypassed.
    #[allow(dead_code)]
    pub constitution_enabled: bool,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let cors_allow_origin =
            std::env::var("CORS_ALLOW_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".into());
        let public_base_url =
            std::env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "http://localhost:3000".into());
        let api_base_url =
            std::env::var("API_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());
        let session_cookie_secure = parse_or(
            "SESSION_COOKIE_SECURE",
            default_session_cookie_secure(&public_base_url, &api_base_url, &cors_allow_origin),
        )?;

        let cfg = Self {
            database_url: required("DATABASE_URL")?,
            jwt_secret: required("JWT_SECRET")?,
            jwt_expiry_hours: parse_or("JWT_EXPIRY_HOURS", 24)?,
            session_idle_timeout_minutes: parse_or("SESSION_IDLE_TIMEOUT_MINUTES", 30)?,
            host: std::env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: parse_or("API_PORT", 8080)?,

            openrouter_api_key: std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
            openrouter_base_url: std::env::var("OPENROUTER_BASE_URL")
                .unwrap_or_else(|_| "https://openrouter.ai/api/v1".into()),

            // OpenRouter slugs use dotted version numbers and the `~` prefix
            // is the "latest-pointer" alias. Defaults pick cost-conscious
            // models — strategist (DeepSeek) and critic (OpenAI) stay in
            // different families so the critic pass is genuinely adversarial,
            // not a self-edit. Claude slugs (`anthropic/claude-opus-4.7`,
            // `~anthropic/claude-sonnet-latest`) remain valid env overrides
            // for any route at ~10–50× the per-token cost.
            model_regime: std::env::var("MODEL_REGIME")
                .unwrap_or_else(|_| "qwen/qwen3.5-flash-02-23".into()),
            // F-COST-1: default to deepseek-v4-flash. The previous default
            // `deepseek/deepseek-v4-pro` had a promo price ($0.435/$0.87 per
            // Mtok) that expired 2026-05-31 → 4× cliff to $1.74/$3.48.
            // v4-flash is permanently $0.112/$0.224 per Mtok with the same
            // 1M context and strong-enough reasoning for this use case.
            model_strategist: std::env::var("MODEL_STRATEGIST")
                .unwrap_or_else(|_| "deepseek/deepseek-v4-flash".into()),
            model_critic: std::env::var("MODEL_CRITIC")
                .unwrap_or_else(|_| "~openai/gpt-mini-latest".into()),
            model_tax: std::env::var("MODEL_TAX").unwrap_or_else(|_| "qwen/qwen3.6-flash".into()),
            model_commentary: std::env::var("MODEL_COMMENTARY")
                .unwrap_or_else(|_| "~google/gemini-flash-latest".into()),

            openrouter_app_name: std::env::var("OPENROUTER_APP_NAME")
                .unwrap_or_else(|_| "Aegis".into()),
            openrouter_app_url: std::env::var("OPENROUTER_APP_URL").ok(),

            coingecko_api_key: std::env::var("COINGECKO_API_KEY").ok(),

            price_provider_primary: std::env::var("PRICE_PROVIDER_PRIMARY")
                .unwrap_or_else(|_| "defillama".into()),
            price_provider_fallback: std::env::var("PRICE_PROVIDER_FALLBACK")
                .unwrap_or_else(|_| "pyth".into()),

            sse_price_tick_secs: parse_or("SSE_PRICE_TICK_SECS", 5)?,

            // Circle is optional in dev (covered by MOCK_CIRCLE) so we don't
            // require it. Production env enforces it via deployment config.
            circle_api_key: std::env::var("CIRCLE_API_KEY").unwrap_or_default(),
            circle_base_url: std::env::var("CIRCLE_BASE_URL")
                .unwrap_or_else(|_| "https://api.circle.com".into()),
            circle_env: std::env::var("CIRCLE_ENV").unwrap_or_else(|_| "sandbox".into()),
            circle_wallet_set_id: std::env::var("CIRCLE_WALLET_SET_ID").unwrap_or_default(),
            circle_entity_secret: std::env::var("CIRCLE_ENTITY_SECRET").unwrap_or_default(),
            circle_mock: parse_or("MOCK_CIRCLE", false)?,

            arc_rpc_url: std::env::var("ARC_RPC_URL")
                .unwrap_or_else(|_| "https://testnet.arc.network".into()),
            base_rpc_url: std::env::var("BASE_RPC_URL")
                .unwrap_or_else(|_| "https://sepolia.base.org".into()),
            eth_rpc_url: std::env::var("ETH_RPC_URL").unwrap_or_default(),
            arb_rpc_url: std::env::var("ARB_RPC_URL").unwrap_or_default(),
            avax_rpc_url: std::env::var("AVAX_RPC_URL").unwrap_or_default(),
            op_rpc_url: std::env::var("OP_RPC_URL").unwrap_or_default(),

            gateway_poll_secs: parse_or("GATEWAY_POLL_SECS", 10)?,
            faucet_max_usdc_per_day: parse_or("FAUCET_MAX_USDC_PER_DAY", 100.0)?,

            cors_allow_origin,
            session_cookie_name: std::env::var("SESSION_COOKIE_NAME")
                .unwrap_or_else(|_| default_session_cookie_name(session_cookie_secure).into()),
            session_cookie_secure,

            cctp_attestation_url: std::env::var("CCTP_ATTESTATION_URL")
                .unwrap_or_else(|_| "https://iris-api-sandbox.circle.com".into()),
            cctp_attestation_timeout_secs: parse_or("CCTP_ATTESTATION_TIMEOUT_SECS", 180)?,
            chain_private_key_arc: std::env::var("CHAIN_PRIVATE_KEY_ARC").unwrap_or_default(),
            chain_private_key_base: std::env::var("CHAIN_PRIVATE_KEY_BASE").unwrap_or_default(),
            chain_private_key_eth: std::env::var("CHAIN_PRIVATE_KEY_ETH").unwrap_or_default(),
            chain_private_key_arb: std::env::var("CHAIN_PRIVATE_KEY_ARB").unwrap_or_default(),
            chain_private_key_avax: std::env::var("CHAIN_PRIVATE_KEY_AVAX").unwrap_or_default(),
            chain_private_key_op: std::env::var("CHAIN_PRIVATE_KEY_OP").unwrap_or_default(),

            // Real execution addresses (only used when EXECUTION_MOCK=false and real-cctp feature)
            cctp_token_messenger_arc: std::env::var("CCTP_TOKEN_MESSENGER_ARC").unwrap_or_default(),
            cctp_token_messenger_base: std::env::var("CCTP_TOKEN_MESSENGER_BASE")
                .unwrap_or_default(),
            cctp_token_messenger_eth: std::env::var("CCTP_TOKEN_MESSENGER_ETH").unwrap_or_default(),
            cctp_token_messenger_arb: std::env::var("CCTP_TOKEN_MESSENGER_ARB").unwrap_or_default(),
            cctp_token_messenger_avax: std::env::var("CCTP_TOKEN_MESSENGER_AVAX")
                .unwrap_or_default(),
            cctp_token_messenger_op: std::env::var("CCTP_TOKEN_MESSENGER_OP").unwrap_or_default(),
            cctp_message_transmitter_arc: std::env::var("CCTP_MESSAGE_TRANSMITTER_ARC")
                .unwrap_or_default(),
            cctp_message_transmitter_base: std::env::var("CCTP_MESSAGE_TRANSMITTER_BASE")
                .unwrap_or_default(),
            cctp_message_transmitter_eth: std::env::var("CCTP_MESSAGE_TRANSMITTER_ETH")
                .unwrap_or_default(),
            cctp_message_transmitter_arb: std::env::var("CCTP_MESSAGE_TRANSMITTER_ARB")
                .unwrap_or_default(),
            cctp_message_transmitter_avax: std::env::var("CCTP_MESSAGE_TRANSMITTER_AVAX")
                .unwrap_or_default(),
            cctp_message_transmitter_op: std::env::var("CCTP_MESSAGE_TRANSMITTER_OP")
                .unwrap_or_default(),
            rebalance_executor_arc: std::env::var("REBALANCE_EXECUTOR_ARC").unwrap_or_default(),
            rebalance_executor_base: std::env::var("REBALANCE_EXECUTOR_BASE").unwrap_or_default(),
            usdc_arc: std::env::var("USDC_ARC").unwrap_or_default(),
            usdc_base: std::env::var("USDC_BASE").unwrap_or_default(),
            usdc_eth: std::env::var("USDC_ETH").unwrap_or_default(),
            usdc_arb: std::env::var("USDC_ARB").unwrap_or_default(),
            usdc_avax: std::env::var("USDC_AVAX").unwrap_or_default(),
            usdc_op: std::env::var("USDC_OP").unwrap_or_default(),
            usyc_token_arc: std::env::var("USYC_TOKEN_ARC").unwrap_or_default(),
            usyc_teller_arc: std::env::var("USYC_TELLER_ARC").unwrap_or_default(),
            usyc_oracle_arc: std::env::var("USYC_ORACLE_ARC").unwrap_or_default(),
            usyc_enabled: parse_or("USYC_ENABLED", false)?,

            uniswap_v3_quoter_base: std::env::var("UNISWAP_V3_QUOTER_BASE").unwrap_or_default(),
            uniswap_v3_router_base: std::env::var("UNISWAP_V3_ROUTER_BASE").unwrap_or_default(),
            weth_base: std::env::var("WETH_BASE").unwrap_or_default(),
            cbbtc_base: std::env::var("CBBTC_BASE").unwrap_or_default(),
            cbeth_base: std::env::var("CBETH_BASE").unwrap_or_default(),
            susds_base: std::env::var("SUSDS_BASE").unwrap_or_default(),

            // Nanopayments (x402) for protocol fee (25bps) and referral payouts.
            nanopayments_facilitator_url: std::env::var("NANOPAYMENTS_FACILITATOR_URL")
                .unwrap_or_else(|_| "https://gateway-api-testnet.circle.com".into()),
            nanopayments_seller_address: std::env::var("NANOPAYMENTS_SELLER_ADDRESS")
                .unwrap_or_default(),
            nanopayments_treasury_address: std::env::var("NANOPAYMENTS_TREASURY_ADDRESS")
                .unwrap_or_default(),

            billing_v2_enabled: parse_or("BILLING_V2_ENABLED", false)?,
            admin_user_ids: std::env::var("ADMIN_USER_IDS")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.trim().is_empty())
                .filter_map(|s| s.trim().parse::<uuid::Uuid>().ok())
                .collect(),

            execution_mock: parse_or("EXECUTION_MOCK", false)?,
            circle_wallet_exec: parse_or("CIRCLE_WALLET_EXEC", false)?,

            scheduler_tick_secs: parse_or("SCHEDULER_TICK_SECS", 300)?,
            scheduler_cooldown_secs: parse_or("SCHEDULER_COOLDOWN_SECS", 1800)?,
            harvest_threshold_usd: parse_or("HARVEST_THRESHOLD_USD", 50.0)?,
            openrouter_budget_guard_usd: parse_or("OPENROUTER_BUDGET_GUARD_USD", 0.05)?,
            stablefx_institutional_access: parse_or("STABLEFX_INSTITUTIONAL_ACCESS", false)?,

            digest_hour_utc: parse_or("DIGEST_HOUR_UTC", 8)?,
            resend_api_key: std::env::var("RESEND_API_KEY").unwrap_or_default(),
            digest_from: std::env::var("DIGEST_FROM")
                .unwrap_or_else(|_| "Aegis <noreply@aegis.local>".into()),
            digest_secret: std::env::var("DIGEST_SECRET")
                .unwrap_or_else(|_| "dev-digest-secret-change-me".into()),
            public_base_url,
            api_base_url,

            regime_backtest_enabled: parse_or("REGIME_BACKTEST_ENABLED", true)?,
            peg_defense_enabled: parse_or("PEG_DEFENSE_ENABLED", true)?,
            peg_monitor_tick_secs: parse_or("PEG_MONITOR_TICK_SECS", 10)?,
            peg_fire_cooldown_secs: parse_or("PEG_FIRE_COOLDOWN_SECS", 1800)?,
            tax_export_v1_enabled: parse_or("TAX_EXPORT_V1_ENABLED", true)?,
            aum_stream_enabled: parse_or("AUM_STREAM_ENABLED", false)?,
            calibrated_conf_enabled: parse_or("CALIBRATED_CONF_ENABLED", false)?,
            constitution_enabled: parse_or("CONSTITUTION_ENABLED", false)?,
        };

        cfg.validate()
            .context("Config::from_env post-construction validation failed")?;
        Ok(cfg)
    }

    /// Production-readiness checks: blow up at boot if a mock flag is OFF but
    /// the required real-world credential is empty. Keeps dev/test ergonomic
    /// while preventing a deploy from silently running with placeholder keys.
    pub fn validate(&self) -> anyhow::Result<()> {
        if !self.execution_mock {
            if self.chain_private_key_arc.trim().is_empty() {
                anyhow::bail!(
                    "EXECUTION_MOCK=false but CHAIN_PRIVATE_KEY_ARC is empty; set it or flip EXECUTION_MOCK=true"
                );
            }
            if self.chain_private_key_base.trim().is_empty() {
                anyhow::bail!(
                    "EXECUTION_MOCK=false but CHAIN_PRIVATE_KEY_BASE is empty; set it or flip EXECUTION_MOCK=true"
                );
            }
        }
        if !self.circle_mock {
            if self.circle_api_key.trim().is_empty() {
                anyhow::bail!(
                    "MOCK_CIRCLE=false but CIRCLE_API_KEY is empty; set it or flip MOCK_CIRCLE=true"
                );
            }
            if self.circle_wallet_set_id.trim().is_empty() {
                anyhow::bail!(
                    "MOCK_CIRCLE=false but CIRCLE_WALLET_SET_ID is empty; set it or flip MOCK_CIRCLE=true"
                );
            }
            if self.circle_entity_secret.trim().is_empty() {
                anyhow::bail!(
                    "MOCK_CIRCLE=false but CIRCLE_ENTITY_SECRET is empty; set it or flip MOCK_CIRCLE=true"
                );
            }
        }
        if self.session_idle_timeout_minutes == 0 {
            anyhow::bail!("SESSION_IDLE_TIMEOUT_MINUTES must be greater than 0");
        }
        if self.session_idle_timeout_minutes > self.jwt_expiry_hours.saturating_mul(60) {
            anyhow::bail!(
                "SESSION_IDLE_TIMEOUT_MINUTES cannot exceed JWT_EXPIRY_HOURS converted to minutes"
            );
        }
        if self.billing_v2_enabled {
            if self.nanopayments_seller_address.trim().is_empty() {
                anyhow::bail!("BILLING_V2_ENABLED=true but NANOPAYMENTS_SELLER_ADDRESS is empty");
            }
            if self.nanopayments_treasury_address.trim().is_empty() {
                anyhow::bail!("BILLING_V2_ENABLED=true but NANOPAYMENTS_TREASURY_ADDRESS is empty");
            }
        }
        // If we will actually send mail, the unsubscribe-token secret must not
        // be the publicly-checked-in default.
        if !self.resend_api_key.trim().is_empty()
            && self.digest_secret == "dev-digest-secret-change-me"
        {
            anyhow::bail!(
                "RESEND_API_KEY is set but DIGEST_SECRET is still the dev default; rotate DIGEST_SECRET before sending real mail"
            );
        }
        // The AUM streamer reads `subscriptions` + `plan_tiers` rows that
        // only exist behind the V2 billing schema. Fail fast at boot rather
        // than silently NULL-rolling on every tick.
        if self.aum_stream_enabled && !self.billing_v2_enabled {
            anyhow::bail!(
                "AUM_STREAM_ENABLED=true requires BILLING_V2_ENABLED=true (depends on subscription rows)"
            );
        }

        // Production deploy hygiene: secure-cookie deploys need a real CORS
        // origin (not the localhost dev fallback).
        if self.session_cookie_secure
            && self
                .cors_allow_origin
                .split(',')
                .all(|o| o.trim().starts_with("http://localhost"))
        {
            anyhow::bail!(
                "SESSION_COOKIE_SECURE=true requires a non-localhost CORS_ALLOW_ORIGIN; set it to your public frontend URL"
            );
        }
        if self.session_cookie_secure && !self.session_cookie_name.starts_with("__Host-") {
            anyhow::bail!(
                "SESSION_COOKIE_SECURE=true requires SESSION_COOKIE_NAME to start with __Host-"
            );
        }
        if !self.session_cookie_secure && self.session_cookie_name.starts_with("__Host-") {
            anyhow::bail!(
                "SESSION_COOKIE_NAME cannot start with __Host- unless SESSION_COOKIE_SECURE=true"
            );
        }
        Ok(())
    }

    /// Resolve a `ModelRoute` to its configured OpenRouter slug.
    pub fn model_for(&self, route: ModelRoute) -> &str {
        match route {
            ModelRoute::RegimeClassify => &self.model_regime,
            ModelRoute::RebalanceReason => &self.model_strategist,
            ModelRoute::TaxExplain => &self.model_tax,
            ModelRoute::MarketCommentary => &self.model_commentary,
            ModelRoute::CritiqueAgent => &self.model_critic,
        }
    }

    /// JSON-RPC URL for `chain`. Empty for chains that have no RPC configured
    /// (the new CCTP testnets default empty until wired).
    #[allow(dead_code)]
    pub fn rpc_url_for(&self, chain: ChainKey) -> &str {
        match chain {
            ChainKey::Arc => &self.arc_rpc_url,
            ChainKey::Base => &self.base_rpc_url,
            ChainKey::EthSepolia => &self.eth_rpc_url,
            ChainKey::ArbSepolia => &self.arb_rpc_url,
            ChainKey::AvaxFuji => &self.avax_rpc_url,
            ChainKey::OpSepolia => &self.op_rpc_url,
        }
    }

    /// EOA signing key (hex) for `chain`. Empty until the chain is funded/wired.
    #[allow(dead_code)]
    pub fn chain_private_key_for(&self, chain: ChainKey) -> &str {
        match chain {
            ChainKey::Arc => &self.chain_private_key_arc,
            ChainKey::Base => &self.chain_private_key_base,
            ChainKey::EthSepolia => &self.chain_private_key_eth,
            ChainKey::ArbSepolia => &self.chain_private_key_arb,
            ChainKey::AvaxFuji => &self.chain_private_key_avax,
            ChainKey::OpSepolia => &self.chain_private_key_op,
        }
    }

    /// CCTP V2 TokenMessenger address for `chain`.
    #[allow(dead_code)]
    pub fn cctp_token_messenger_for(&self, chain: ChainKey) -> &str {
        match chain {
            ChainKey::Arc => &self.cctp_token_messenger_arc,
            ChainKey::Base => &self.cctp_token_messenger_base,
            ChainKey::EthSepolia => &self.cctp_token_messenger_eth,
            ChainKey::ArbSepolia => &self.cctp_token_messenger_arb,
            ChainKey::AvaxFuji => &self.cctp_token_messenger_avax,
            ChainKey::OpSepolia => &self.cctp_token_messenger_op,
        }
    }

    /// CCTP V2 MessageTransmitter address for `chain`.
    #[allow(dead_code)]
    pub fn cctp_message_transmitter_for(&self, chain: ChainKey) -> &str {
        match chain {
            ChainKey::Arc => &self.cctp_message_transmitter_arc,
            ChainKey::Base => &self.cctp_message_transmitter_base,
            ChainKey::EthSepolia => &self.cctp_message_transmitter_eth,
            ChainKey::ArbSepolia => &self.cctp_message_transmitter_arb,
            ChainKey::AvaxFuji => &self.cctp_message_transmitter_avax,
            ChainKey::OpSepolia => &self.cctp_message_transmitter_op,
        }
    }

    /// USDC ERC-20 address for `chain`.
    #[allow(dead_code)]
    pub fn usdc_for(&self, chain: ChainKey) -> &str {
        match chain {
            ChainKey::Arc => &self.usdc_arc,
            ChainKey::Base => &self.usdc_base,
            ChainKey::EthSepolia => &self.usdc_eth,
            ChainKey::ArbSepolia => &self.usdc_arb,
            ChainKey::AvaxFuji => &self.usdc_avax,
            ChainKey::OpSepolia => &self.usdc_op,
        }
    }

    /// Destination-chain RebalanceExecutor address (the CCTP mintRecipient for a
    /// hook-enabled burn). Only Arc and Base have a deployed executor today; the
    /// rest return empty and fail closed before any burn is attempted.
    #[allow(dead_code)]
    pub fn rebalance_executor_for(&self, chain: ChainKey) -> &str {
        match chain {
            ChainKey::Arc => &self.rebalance_executor_arc,
            ChainKey::Base => &self.rebalance_executor_base,
            ChainKey::EthSepolia
            | ChainKey::ArbSepolia
            | ChainKey::AvaxFuji
            | ChainKey::OpSepolia => "",
        }
    }
}

fn required(key: &str) -> anyhow::Result<String> {
    std::env::var(key).with_context(|| format!("missing required env var: {key}"))
}

fn parse_or<T: std::str::FromStr>(key: &str, default: T) -> anyhow::Result<T>
where
    T::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Ok(v) => v
            .parse()
            .map_err(|e| anyhow::anyhow!("{key} must parse: {e}")),
        Err(_) => Ok(default),
    }
}

fn default_session_cookie_secure(
    public_base_url: &str,
    api_base_url: &str,
    cors_allow_origin: &str,
) -> bool {
    let origins = std::iter::once(public_base_url)
        .chain(std::iter::once(api_base_url))
        .chain(cors_allow_origin.split(','));
    !origins
        .filter(|origin| !origin.trim().is_empty())
        .all(is_local_http_origin)
}

fn default_session_cookie_name(secure: bool) -> &'static str {
    if secure {
        "__Host-aegis_session"
    } else {
        "aegis_session"
    }
}

fn is_local_http_origin(origin: &str) -> bool {
    let origin = origin.trim();
    origin.starts_with("http://localhost")
        || origin.starts_with("http://127.0.0.1")
        || origin.starts_with("http://[::1]")
}

/// Test-only fully-populated config (mock mode), shared by unit tests across
/// modules. Lives at module scope so other modules' `#[cfg(test)]` blocks can
/// build a realistic `Config` via `crate::config::test_config()`.
#[cfg(test)]
pub(crate) fn test_config() -> Config {
    Config {
        database_url: "postgres://test".into(),
        jwt_secret: "secret".into(),
        jwt_expiry_hours: 24,
        session_idle_timeout_minutes: 30,
        host: "0.0.0.0".into(),
        port: 8080,
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
        price_provider_fallback: "pyth".into(),
        sse_price_tick_secs: 5,
        circle_api_key: "circle-key".into(),
        circle_base_url: "https://api.circle.com".into(),
        circle_env: "sandbox".into(),
        circle_wallet_set_id: "00000000-0000-4000-8000-000000000000".into(),
        circle_entity_secret: "0000000000000000000000000000000000000000000000000000000000000000"
            .into(),
        circle_mock: true,
        arc_rpc_url: "https://testnet.arc.network".into(),
        base_rpc_url: "https://sepolia.base.org".into(),
        eth_rpc_url: String::new(),
        arb_rpc_url: String::new(),
        avax_rpc_url: String::new(),
        op_rpc_url: String::new(),
        gateway_poll_secs: 10,
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
        nanopayments_facilitator_url: "https://gateway-api-testnet.circle.com".into(),
        nanopayments_seller_address: String::new(),
        nanopayments_treasury_address: String::new(),
        billing_v2_enabled: false,
        admin_user_ids: vec![],
        execution_mock: true,
        circle_wallet_exec: false,
        scheduler_tick_secs: 300,
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
        peg_monitor_tick_secs: 10,
        peg_fire_cooldown_secs: 1800,
        tax_export_v1_enabled: true,
        aum_stream_enabled: false,
        calibrated_conf_enabled: false,
        constitution_enabled: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_aum_stream_without_billing_v2() {
        let mut cfg = test_config();
        cfg.aum_stream_enabled = true;
        cfg.billing_v2_enabled = false;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_accepts_aum_stream_with_billing_v2() {
        let mut cfg = test_config();
        cfg.aum_stream_enabled = true;
        cfg.billing_v2_enabled = true;
        // A1's stricter validation also requires the Nanopayments
        // addresses to be set whenever BILLING_V2_ENABLED=true.
        cfg.nanopayments_seller_address = "0xseller".into();
        cfg.nanopayments_treasury_address = "0xtreasury".into();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_rejects_real_circle_without_wallet_set() {
        let mut cfg = test_config();
        cfg.circle_mock = false;
        cfg.circle_wallet_set_id = String::new();
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("CIRCLE_WALLET_SET_ID"));
    }

    #[test]
    fn validate_rejects_real_circle_without_entity_secret() {
        let mut cfg = test_config();
        cfg.circle_mock = false;
        cfg.circle_entity_secret = String::new();
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("CIRCLE_ENTITY_SECRET"));
    }

    #[test]
    fn model_for_resolves_each_route() {
        let cfg = test_config();
        assert_eq!(cfg.model_for(ModelRoute::RegimeClassify), "regime-model");
        assert_eq!(
            cfg.model_for(ModelRoute::RebalanceReason),
            "strategist-model"
        );
        assert_eq!(cfg.model_for(ModelRoute::CritiqueAgent), "critic-model");
        assert_eq!(cfg.model_for(ModelRoute::TaxExplain), "tax-model");
        assert_eq!(
            cfg.model_for(ModelRoute::MarketCommentary),
            "commentary-model"
        );
    }

    #[test]
    fn secure_cookie_default_stays_off_for_localhost_dev() {
        assert!(!default_session_cookie_secure(
            "http://localhost:3000",
            "http://localhost:8080",
            "http://localhost:3000"
        ));
    }

    #[test]
    fn secure_cookie_default_turns_on_for_public_origins() {
        assert!(default_session_cookie_secure(
            "https://aegis.example",
            "https://api.aegis.example",
            "https://aegis.example"
        ));
        assert!(default_session_cookie_secure(
            "http://localhost:3000",
            "http://localhost:8080",
            "https://aegis.example"
        ));
    }

    #[test]
    fn validate_requires_host_prefixed_name_for_secure_cookie() {
        let mut cfg = test_config();
        cfg.session_cookie_secure = true;
        cfg.cors_allow_origin = "https://aegis.example".into();
        cfg.session_cookie_name = "aegis_session".into();
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("__Host-"));

        cfg.session_cookie_name = "__Host-aegis_session".into();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_rejects_host_prefixed_cookie_without_secure_flag() {
        let mut cfg = test_config();
        cfg.session_cookie_secure = false;
        cfg.session_cookie_name = "__Host-aegis_session".into();
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("SESSION_COOKIE_SECURE=true"));
    }
}
