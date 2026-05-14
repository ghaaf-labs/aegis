use anyhow::Context;

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
    pub jwt_secret: String,
    pub jwt_expiry_hours: u64,
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

    /// Cadence (seconds) for the SSE price ticker. Lower = more "realtime"
    /// feel; higher = friendlier to upstream rate limits.
    pub sse_price_tick_secs: u64,

    // ── Circle (Wallets, Gateway, Paymaster, USYC, StableFX) ──────────────
    pub circle_api_key: String,
    pub circle_base_url: String,
    #[allow(dead_code)]
    pub circle_env: String,
    /// When true, the wallet module uses an in-process mock provider instead
    /// of hitting Circle WaaS. Keeps local dev moving when the sandbox is
    /// unreachable or when running CI without a key.
    pub circle_mock: bool,

    #[allow(dead_code)]
    pub arc_rpc_url: String,
    #[allow(dead_code)]
    pub base_rpc_url: String,

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

    /// Cookie name for the JWT.
    pub session_cookie_name: String,

    /// When true, the `Secure` flag is set on the auth cookie. Default true
    /// in production; flip to false for `http://localhost` dev.
    pub session_cookie_secure: bool,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            jwt_secret: required("JWT_SECRET")?,
            jwt_expiry_hours: parse_or("JWT_EXPIRY_HOURS", 24)?,
            host: std::env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: parse_or("API_PORT", 8080)?,

            openrouter_api_key: required("OPENROUTER_API_KEY")?,
            openrouter_base_url: std::env::var("OPENROUTER_BASE_URL")
                .unwrap_or_else(|_| "https://openrouter.ai/api/v1".into()),

            model_regime: std::env::var("MODEL_REGIME")
                .unwrap_or_else(|_| "anthropic/claude-haiku-4-5".into()),
            model_strategist: std::env::var("MODEL_STRATEGIST")
                .unwrap_or_else(|_| "anthropic/claude-opus-4-7".into()),
            model_critic: std::env::var("MODEL_CRITIC").unwrap_or_else(|_| "openai/gpt-5".into()),
            model_tax: std::env::var("MODEL_TAX")
                .unwrap_or_else(|_| "anthropic/claude-sonnet-4-6".into()),
            model_commentary: std::env::var("MODEL_COMMENTARY")
                .unwrap_or_else(|_| "google/gemini-2.5-flash".into()),

            openrouter_app_name: std::env::var("OPENROUTER_APP_NAME")
                .unwrap_or_else(|_| "Aegis".into()),
            openrouter_app_url: std::env::var("OPENROUTER_APP_URL").ok(),

            coingecko_api_key: std::env::var("COINGECKO_API_KEY").ok(),

            sse_price_tick_secs: parse_or("SSE_PRICE_TICK_SECS", 5)?,

            // Circle is optional in dev (covered by MOCK_CIRCLE) so we don't
            // require it. Production env enforces it via deployment config.
            circle_api_key: std::env::var("CIRCLE_API_KEY").unwrap_or_default(),
            circle_base_url: std::env::var("CIRCLE_BASE_URL")
                .unwrap_or_else(|_| "https://api.circle.com".into()),
            circle_env: std::env::var("CIRCLE_ENV").unwrap_or_else(|_| "sandbox".into()),
            circle_mock: parse_or("MOCK_CIRCLE", true)?,

            arc_rpc_url: std::env::var("ARC_RPC_URL")
                .unwrap_or_else(|_| "https://testnet.arc.network".into()),
            base_rpc_url: std::env::var("BASE_RPC_URL")
                .unwrap_or_else(|_| "https://sepolia.base.org".into()),

            gateway_poll_secs: parse_or("GATEWAY_POLL_SECS", 10)?,
            faucet_max_usdc_per_day: parse_or("FAUCET_MAX_USDC_PER_DAY", 100.0)?,

            cors_allow_origin: std::env::var("CORS_ALLOW_ORIGIN")
                .unwrap_or_else(|_| "http://localhost:3000".into()),
            session_cookie_name: std::env::var("SESSION_COOKIE_NAME")
                .unwrap_or_else(|_| "aegis_jwt".into()),
            session_cookie_secure: parse_or("SESSION_COOKIE_SECURE", false)?,
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            database_url: "postgres://test".into(),
            jwt_secret: "secret".into(),
            jwt_expiry_hours: 24,
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
            sse_price_tick_secs: 5,
            circle_api_key: "circle-key".into(),
            circle_base_url: "https://api.circle.com".into(),
            circle_env: "sandbox".into(),
            circle_mock: true,
            arc_rpc_url: "https://testnet.arc.network".into(),
            base_rpc_url: "https://sepolia.base.org".into(),
            gateway_poll_secs: 10,
            faucet_max_usdc_per_day: 100.0,
            cors_allow_origin: "http://localhost:3000".into(),
            session_cookie_name: "aegis_jwt".into(),
            session_cookie_secure: false,
        }
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
}
