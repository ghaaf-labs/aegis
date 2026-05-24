use anyhow::Context;

use crate::domain::ChainKey;

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

/// Per-chain settlement infrastructure. One instance per [`ChainKey`], reached
/// through [`Config::chain`]. Every address default-empty ⇒ the route registry
/// fails that leg closed (it never executes against a blank address).
#[derive(Debug, Clone, Default)]
pub struct ChainConfig {
    pub rpc_url: String,
    pub private_key: String,
    pub cctp_token_messenger: String,
    pub cctp_message_transmitter: String,
    pub usdc: String,
    pub rebalance_executor: String, // "" for chains without a deployed executor
    pub swap_router: String,        // "" for chains with no AMM venue
    pub swap_quoter: String,
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

    // Per-route model slugs. Each value is an ordered, comma-separated
    // fallback chain: `primary[,fallback,…]`. OpenRouter tries them in order
    // and falls back on a provider 5xx/429/refusal (see `models_for`). A bare
    // single slug stays valid (a one-element chain). Defaults below; override
    // via env.
    pub model_regime: String,
    pub model_strategist: String,
    pub model_critic: String,
    pub model_tax: String,
    pub model_commentary: String,

    // Per-attempt resilience for OpenRouter calls. `max_retries` are reqwest-
    // level retries on a *transient* failure (timeout / 5xx / 429), on top of
    // the in-request `models[]` fallback; `attempt_timeout_secs` caps a single
    // attempt (shorter than reqwest's 240s ceiling) so a stalled provider is
    // abandoned and retried instead of hanging. `response_healing` toggles
    // OpenRouter's JSON-repair plugin.
    pub openrouter_max_retries: u32,
    pub openrouter_attempt_timeout_secs: u64,
    pub openrouter_response_healing: bool,

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

    /// Per-chain settlement infrastructure (RPC, signer, CCTP V2 contracts,
    /// USDC, executor, swap venue), one entry per [`ChainKey`] indexed by
    /// [`ChainKey::index`]. Accessed via [`Config::chain`]. Per-(token×chain)
    /// ERC-20s (`weth_*`, `wbtc_*`, …) stay flat below — they are not chain
    /// infra and are out of this collection.
    #[allow(dead_code)]
    pub chains: [ChainConfig; 6],

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

    // ── Per-(symbol, chain) ERC-20 addresses ───────────────────────────────
    /// Built in `from_env` by iterating the token registry × its `Env`-sourced
    /// residencies and reading `{PREFIX}_{CHAIN}` (e.g. `WETH_BASE`, `WBTC_ETH`,
    /// `CBBTC_BASE`, `LINK_BASE`). USDC (per-chain `ChainConfig`) and USYC
    /// (`usyc_token_arc`) are resolved separately and are NOT in this map. Empty
    /// / zero values are kept verbatim and normalized to `None` at read time by
    /// `TokenSpec::address_for`. Resolve via [`Config::token_address`]; mutate in
    /// tests via `Config::set_token_address`.
    pub token_addrs: std::collections::HashMap<(&'static str, ChainKey), String>,

    // ── Per-chain swap-venue liquidity allowlist ──────────────────────────
    /// Symbols that have a tradeable swap pool with real liquidity on a given
    /// chain. Built in `from_env` from `SWAP_LIQUID_TOKENS_{CHAIN}` (e.g.
    /// `SWAP_LIQUID_TOKENS_BASE=ETH`). When set, the list is authoritative.
    /// When absent, Aegis uses a tiny curated testnet default instead of
    /// trusting every configured ERC-20; addresses alone do not prove liquidity.
    /// Resolve via [`Config::swap_token_has_venue`].
    pub swap_liquid_tokens: std::collections::HashMap<ChainKey, Vec<String>>,

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
            // model_bench (2026-05-24, on the real allocator task) measured the
            // candidates: gemini-flash p50 7.9s / p95 8.7s @ 100% valid; sonnet
            // p50 8.8s @ 100%; deepseek p50 8.9s @ 100% (its prior 38–240s was a
            // transient provider stall, not inherent) — and qwen3.5-flash the
            // actual laggard at p50 25s / p95 40s @ 83% (one timeout). So both
            // hot paths now LEAD with the fastest reliable model and keep
            // cross-vendor fallbacks: OpenRouter falls back on a 5xx/429/refusal
            // so a single bad route can't stall or 500 the decision.
            model_regime: std::env::var("MODEL_REGIME")
                .unwrap_or_else(|_| "~google/gemini-flash-latest,qwen/qwen3.5-flash-02-23".into()),
            model_strategist: std::env::var("MODEL_STRATEGIST").unwrap_or_else(|_| {
                "~google/gemini-flash-latest,~anthropic/claude-sonnet-latest,deepseek/deepseek-v4-flash"
                    .into()
            }),
            model_critic: std::env::var("MODEL_CRITIC")
                .unwrap_or_else(|_| "~openai/gpt-mini-latest".into()),
            model_tax: std::env::var("MODEL_TAX").unwrap_or_else(|_| "qwen/qwen3.6-flash".into()),
            model_commentary: std::env::var("MODEL_COMMENTARY")
                .unwrap_or_else(|_| "~google/gemini-flash-latest".into()),

            openrouter_app_name: std::env::var("OPENROUTER_APP_NAME")
                .unwrap_or_else(|_| "Aegis".into()),
            openrouter_app_url: std::env::var("OPENROUTER_APP_URL").ok(),

            openrouter_max_retries: parse_or("OPENROUTER_MAX_RETRIES", 1)?,
            openrouter_attempt_timeout_secs: parse_or("OPENROUTER_ATTEMPT_TIMEOUT_SECS", 90)?,
            openrouter_response_healing: parse_or("OPENROUTER_RESPONSE_HEALING", true)?,

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

            chains: chain_configs_from_env(),

            gateway_poll_secs: parse_or("GATEWAY_POLL_SECS", 10)?,
            faucet_max_usdc_per_day: parse_or("FAUCET_MAX_USDC_PER_DAY", 100.0)?,

            cors_allow_origin,
            session_cookie_name: std::env::var("SESSION_COOKIE_NAME")
                .unwrap_or_else(|_| default_session_cookie_name(session_cookie_secure).into()),
            session_cookie_secure,

            cctp_attestation_url: std::env::var("CCTP_ATTESTATION_URL")
                .unwrap_or_else(|_| "https://iris-api-sandbox.circle.com".into()),
            cctp_attestation_timeout_secs: parse_or("CCTP_ATTESTATION_TIMEOUT_SECS", 180)?,
            usyc_token_arc: std::env::var("USYC_TOKEN_ARC").unwrap_or_default(),
            usyc_teller_arc: std::env::var("USYC_TELLER_ARC").unwrap_or_default(),
            usyc_oracle_arc: std::env::var("USYC_ORACLE_ARC").unwrap_or_default(),
            usyc_enabled: parse_or("USYC_ENABLED", false)?,

            token_addrs: token_addrs_from_env(),
            swap_liquid_tokens: swap_liquid_tokens_from_env(),

            // Nanopayments (x402) for protocol fee (25bps) and referral payouts.
            nanopayments_facilitator_url: std::env::var("NANOPAYMENTS_FACILITATOR_URL")
                .unwrap_or_else(|_| "https://gateway-api-testnet.circle.com".into()),
            nanopayments_seller_address: std::env::var("NANOPAYMENTS_SELLER_ADDRESS")
                .unwrap_or_default(),
            nanopayments_treasury_address: std::env::var("NANOPAYMENTS_TREASURY_ADDRESS")
                .unwrap_or_default(),

            billing_v2_enabled: parse_or("BILLING_V2_ENABLED", false)?,
            admin_user_ids: parse_admin_user_ids(
                &std::env::var("ADMIN_USER_IDS").unwrap_or_default(),
            ),

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

            regime_backtest_enabled: parse_or("REGIME_BACKTEST_ENABLED", false)?,
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
            if self.circle_wallet_exec {
                // Non-custodial execution submits CCTP burns/mints/swaps from
                // each user's Circle developer-controlled wallet, so it needs
                // real Circle (validated below under !circle_mock) — not backend
                // EOA signing keys. Requiring CHAIN_PRIVATE_KEY_* here would
                // force operators to hold keys the non-custodial path never uses.
                if self.circle_mock {
                    anyhow::bail!(
                        "CIRCLE_WALLET_EXEC=true with EXECUTION_MOCK=false requires MOCK_CIRCLE=false; it submits from the user's Circle wallet"
                    );
                }
            } else {
                if self.chain(ChainKey::Arc).private_key.trim().is_empty() {
                    anyhow::bail!(
                        "EXECUTION_MOCK=false but CHAIN_PRIVATE_KEY_ARC is empty; set it, enable CIRCLE_WALLET_EXEC, or flip EXECUTION_MOCK=true"
                    );
                }
                if self.chain(ChainKey::Base).private_key.trim().is_empty() {
                    anyhow::bail!(
                        "EXECUTION_MOCK=false but CHAIN_PRIVATE_KEY_BASE is empty; set it, enable CIRCLE_WALLET_EXEC, or flip EXECUTION_MOCK=true"
                    );
                }
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

    /// The raw, possibly-comma-separated chain configured for a route.
    fn model_spec(&self, route: ModelRoute) -> &str {
        match route {
            ModelRoute::RegimeClassify => &self.model_regime,
            ModelRoute::RebalanceReason => &self.model_strategist,
            ModelRoute::TaxExplain => &self.model_tax,
            ModelRoute::MarketCommentary => &self.model_commentary,
            ModelRoute::CritiqueAgent => &self.model_critic,
        }
    }

    /// Resolve a `ModelRoute` to its **primary** OpenRouter slug (the first
    /// entry of the chain). Used for error messages and as the requested-slug
    /// fallback in telemetry; the wire request sends the whole [`models_for`]
    /// chain.
    pub fn model_for(&self, route: ModelRoute) -> &str {
        let raw = self.model_spec(route);
        raw.split(',').next().unwrap_or(raw).trim()
    }

    /// Resolve a `ModelRoute` to its ordered OpenRouter fallback chain
    /// (`primary` first). Empty entries are dropped so trailing commas and
    /// stray whitespace in env overrides are harmless. Always non-empty for a
    /// configured route.
    pub fn models_for(&self, route: ModelRoute) -> Vec<&str> {
        self.model_spec(route)
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Per-chain settlement infrastructure for `chain` — RPC URL, EOA signing
    /// key, CCTP V2 contracts, USDC, executor, and swap venue. The single
    /// accessor over the `chains` collection. Addresses default empty until the
    /// chain is wired (Arc has no AMM venue; only Arc/Base have a deployed
    /// executor) so unconfigured legs fail closed.
    #[allow(dead_code)]
    pub fn chain(&self, chain: ChainKey) -> &ChainConfig {
        &self.chains[chain.index()]
    }

    /// Raw (un-normalized) ERC-20 for `symbol` on `chain` from the flat env-built
    /// token map, or `None` when the token has no `Env`-sourced residency there.
    /// Normalization (empty / zero placeholder → `None`) is applied by the
    /// caller, `TokenSpec::address_for`. The map is small (one entry per
    /// configured token×chain), so a linear scan keeps the key lifetime simple.
    pub fn token_address_raw(&self, symbol: &str, chain: ChainKey) -> Option<&str> {
        self.token_addrs
            .iter()
            .find(|((sym, ch), _)| *sym == symbol && *ch == chain)
            .map(|(_, addr)| addr.as_str())
    }

    /// Set a token's ERC-20 on a chain — the ergonomic seam that replaces the
    /// old flat `cfg.weth_base = …` field pokes (used by unit + integration
    /// tests, and available for any runtime wiring).
    pub fn set_token_address(&mut self, symbol: &'static str, chain: ChainKey, addr: &str) {
        self.token_addrs.insert((symbol, chain), addr.into());
    }

    /// Whether `symbol` has a tradeable (liquid) swap venue on `chain`. A
    /// configured allowlist is authoritative; an absent/empty one falls back to
    /// the small curated default below. Case-insensitive so
    /// `SWAP_LIQUID_TOKENS_BASE=eth` matches `ETH`.
    pub fn swap_token_has_venue(&self, symbol: &str, chain: ChainKey) -> bool {
        match self.swap_liquid_tokens.get(&chain) {
            Some(list) if !list.is_empty() => list.iter().any(|s| s.eq_ignore_ascii_case(symbol)),
            _ => default_swap_liquid_token(symbol, chain),
        }
    }
}

fn required(key: &str) -> anyhow::Result<String> {
    std::env::var(key).with_context(|| format!("missing required env var: {key}"))
}

/// Build the per-(symbol, chain) ERC-20 map from env: for every registry token,
/// read `{PREFIX}_{CHAIN}` for each `Env(prefix)` residency. Env var names are
/// unchanged (`WETH_BASE`, `WBTC_ETH`, `CBBTC_BASE`, `LINK_BASE`, …). USDC
/// (per-`ChainConfig`) and USYC (`usyc_token_arc`) have their own slots and are
/// not `Env`-sourced, so they're skipped here. Empty values are kept (the
/// registry normalizes them to `None` / track-only at read time).
fn token_addrs_from_env() -> std::collections::HashMap<(&'static str, ChainKey), String> {
    use crate::domain::token::{AddrSource, TOKEN_REGISTRY};
    let mut map = std::collections::HashMap::new();
    for spec in TOKEN_REGISTRY {
        for res in spec.residencies {
            if let AddrSource::Env(prefix) = res.addr {
                let key = format!("{prefix}_{}", chain_env_suffix(res.chain));
                map.insert(
                    (spec.symbol, res.chain),
                    std::env::var(key).unwrap_or_default(),
                );
            }
        }
    }
    map
}

/// Build the per-chain swap-venue liquidity allowlist from
/// `SWAP_LIQUID_TOKENS_{CHAIN}` (comma-separated symbols, e.g.
/// `SWAP_LIQUID_TOKENS_BASE=ETH`). Only chains with a non-empty env var get an
/// entry; missing env falls back to `default_swap_liquid_token`.
fn swap_liquid_tokens_from_env() -> std::collections::HashMap<ChainKey, Vec<String>> {
    let mut map = std::collections::HashMap::new();
    for chain in ChainKey::ALL {
        let key = format!("SWAP_LIQUID_TOKENS_{}", chain_env_suffix(chain));
        if let Ok(raw) = std::env::var(&key) {
            let list: Vec<String> = raw
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !list.is_empty() {
                map.insert(chain, list);
            }
        }
    }
    map
}

fn default_swap_liquid_token(symbol: &str, chain: ChainKey) -> bool {
    chain == ChainKey::Base && symbol.eq_ignore_ascii_case("ETH")
}

/// The env-var chain suffix matching the historical `{PREFIX}_{CHAIN}` names.
fn chain_env_suffix(chain: ChainKey) -> &'static str {
    match chain {
        ChainKey::Arc => "ARC",
        ChainKey::Base => "BASE",
        ChainKey::EthSepolia => "ETH",
        ChainKey::ArbSepolia => "ARB",
        ChainKey::AvaxFuji => "AVAX",
        ChainKey::OpSepolia => "OP",
    }
}

/// Build the per-chain config array from env, indexed by [`ChainKey::index`].
/// Env var names are unchanged (`{KNOB}_{CHAIN}`). The swap venue mapping
/// mirrors the old `swap_router_for`/`swap_quoter_for`: Base = Aerodrome
/// Slipstream, OP = Velodrome, Eth/Arb = Uniswap V3 (all share the V3-style
/// router + QuoterV2 surface, so they read `UNISWAP_V3_{ROUTER,QUOTER}_*`); Avax
/// = Trader Joe LB (its own bin-step `Path` ABI — `TRADER_JOE_LB_*`); Arc has no
/// AMM venue, so its swap addresses stay empty and fail closed. Only Arc/Base
/// have a deployed RebalanceExecutor; the rest leave it empty.
fn chain_configs_from_env() -> [ChainConfig; 6] {
    let env = |key: &str| std::env::var(key).unwrap_or_default();
    [
        // Arc
        ChainConfig {
            rpc_url: std::env::var("ARC_RPC_URL")
                .unwrap_or_else(|_| "https://testnet.arc.network".into()),
            private_key: env("CHAIN_PRIVATE_KEY_ARC"),
            cctp_token_messenger: env("CCTP_TOKEN_MESSENGER_ARC"),
            cctp_message_transmitter: env("CCTP_MESSAGE_TRANSMITTER_ARC"),
            usdc: env("USDC_ARC"),
            rebalance_executor: env("REBALANCE_EXECUTOR_ARC"),
            // Arc settles stables / FX; no AMM venue.
            swap_router: String::new(),
            swap_quoter: String::new(),
        },
        // Base
        ChainConfig {
            rpc_url: std::env::var("BASE_RPC_URL")
                .unwrap_or_else(|_| "https://sepolia.base.org".into()),
            private_key: env("CHAIN_PRIVATE_KEY_BASE"),
            cctp_token_messenger: env("CCTP_TOKEN_MESSENGER_BASE"),
            cctp_message_transmitter: env("CCTP_MESSAGE_TRANSMITTER_BASE"),
            usdc: env("USDC_BASE"),
            rebalance_executor: env("REBALANCE_EXECUTOR_BASE"),
            swap_router: env("UNISWAP_V3_ROUTER_BASE"),
            swap_quoter: env("UNISWAP_V3_QUOTER_BASE"),
        },
        // EthSepolia
        ChainConfig {
            rpc_url: env("ETH_RPC_URL"),
            private_key: env("CHAIN_PRIVATE_KEY_ETH"),
            cctp_token_messenger: env("CCTP_TOKEN_MESSENGER_ETH"),
            cctp_message_transmitter: env("CCTP_MESSAGE_TRANSMITTER_ETH"),
            usdc: env("USDC_ETH"),
            rebalance_executor: String::new(),
            swap_router: env("UNISWAP_V3_ROUTER_ETH"),
            swap_quoter: env("UNISWAP_V3_QUOTER_ETH"),
        },
        // ArbSepolia
        ChainConfig {
            rpc_url: env("ARB_RPC_URL"),
            private_key: env("CHAIN_PRIVATE_KEY_ARB"),
            cctp_token_messenger: env("CCTP_TOKEN_MESSENGER_ARB"),
            cctp_message_transmitter: env("CCTP_MESSAGE_TRANSMITTER_ARB"),
            usdc: env("USDC_ARB"),
            rebalance_executor: String::new(),
            swap_router: env("UNISWAP_V3_ROUTER_ARB"),
            swap_quoter: env("UNISWAP_V3_QUOTER_ARB"),
        },
        // AvaxFuji — Trader Joe LB venue (distinct ABI; dispatched on SwapVenue).
        ChainConfig {
            rpc_url: env("AVAX_RPC_URL"),
            private_key: env("CHAIN_PRIVATE_KEY_AVAX"),
            cctp_token_messenger: env("CCTP_TOKEN_MESSENGER_AVAX"),
            cctp_message_transmitter: env("CCTP_MESSAGE_TRANSMITTER_AVAX"),
            usdc: env("USDC_AVAX"),
            rebalance_executor: String::new(),
            swap_router: env("TRADER_JOE_LB_ROUTER_AVAX"),
            swap_quoter: env("TRADER_JOE_LB_QUOTER_AVAX"),
        },
        // OpSepolia
        ChainConfig {
            rpc_url: env("OP_RPC_URL"),
            private_key: env("CHAIN_PRIVATE_KEY_OP"),
            cctp_token_messenger: env("CCTP_TOKEN_MESSENGER_OP"),
            cctp_message_transmitter: env("CCTP_MESSAGE_TRANSMITTER_OP"),
            usdc: env("USDC_OP"),
            rebalance_executor: String::new(),
            swap_router: env("UNISWAP_V3_ROUTER_OP"),
            swap_quoter: env("UNISWAP_V3_QUOTER_OP"),
        },
    ]
}

/// Parse a comma-separated `ADMIN_USER_IDS` list into UUIDs, warning loudly on
/// any malformed entry instead of silently dropping it (a typo would otherwise
/// quietly demote an operator out of the admin set).
fn parse_admin_user_ids(raw: &str) -> Vec<uuid::Uuid> {
    let mut ids = Vec::new();
    for entry in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        match entry.parse::<uuid::Uuid>() {
            Ok(id) => ids.push(id),
            Err(e) => {
                tracing::warn!(value = %entry, error = %e, "ADMIN_USER_IDS: ignoring invalid UUID");
            }
        }
    }
    ids
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
        openrouter_max_retries: 1,
        openrouter_attempt_timeout_secs: 90,
        openrouter_response_healing: true,
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
        gateway_poll_secs: 10,
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
    fn models_for_parses_fallback_chain_primary_first() {
        let mut cfg = test_config();
        cfg.model_strategist = " a/primary , b/fallback ,, c/last ".into();
        // `model_for` is the trimmed primary; `models_for` is the full chain
        // with blank entries dropped and whitespace trimmed.
        assert_eq!(cfg.model_for(ModelRoute::RebalanceReason), "a/primary");
        assert_eq!(
            cfg.models_for(ModelRoute::RebalanceReason),
            vec!["a/primary", "b/fallback", "c/last"]
        );
        // A bare single slug is a one-element chain.
        assert_eq!(
            cfg.models_for(ModelRoute::CritiqueAgent),
            vec!["critic-model"]
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

    #[test]
    fn swap_venue_helpers_resolve_per_chain() {
        let mut cfg = test_config();
        cfg.chains[ChainKey::Base.index()].swap_router = "0xbase_router".into();
        cfg.chains[ChainKey::Base.index()].swap_quoter = "0xbase_quoter".into();
        cfg.chains[ChainKey::OpSepolia.index()].swap_router = "0xop_router".into();
        cfg.chains[ChainKey::EthSepolia.index()].swap_quoter = "0xeth_quoter".into();
        cfg.chains[ChainKey::AvaxFuji.index()].swap_router = "0xavax_lb_router".into();
        cfg.chains[ChainKey::AvaxFuji.index()].swap_quoter = "0xavax_lb_quoter".into();

        assert_eq!(cfg.chain(ChainKey::Base).swap_router, "0xbase_router");
        assert_eq!(cfg.chain(ChainKey::Base).swap_quoter, "0xbase_quoter");
        assert_eq!(cfg.chain(ChainKey::OpSepolia).swap_router, "0xop_router");
        assert_eq!(cfg.chain(ChainKey::EthSepolia).swap_quoter, "0xeth_quoter");
        // Avax resolves to the Trader Joe LB venue (its own ABI, dispatched in
        // the swap adapter).
        assert_eq!(
            cfg.chain(ChainKey::AvaxFuji).swap_router,
            "0xavax_lb_router"
        );
        assert_eq!(
            cfg.chain(ChainKey::AvaxFuji).swap_quoter,
            "0xavax_lb_quoter"
        );
        // Arc has no AMM venue → always empty.
        assert_eq!(cfg.chain(ChainKey::Arc).swap_router, "");
        assert_eq!(cfg.chain(ChainKey::Arc).swap_quoter, "");
    }
}
