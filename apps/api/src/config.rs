use anyhow::Context;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_expiry_hours: u64,
    pub host: String,
    pub port: u16,
    pub openai_api_key: String,
    pub openai_model: String,
    pub coingecko_api_key: Option<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            jwt_secret: required("JWT_SECRET")?,
            jwt_expiry_hours: std::env::var("JWT_EXPIRY_HOURS")
                .unwrap_or_else(|_| "24".into())
                .parse()
                .context("JWT_EXPIRY_HOURS must be a number")?,
            host: std::env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: std::env::var("API_PORT")
                .unwrap_or_else(|_| "8080".into())
                .parse()
                .context("API_PORT must be a number")?,
            openai_api_key: required("OPENAI_API_KEY")?,
            openai_model: std::env::var("OPENAI_MODEL")
                .unwrap_or_else(|_| "gpt-4o".into()),
            coingecko_api_key: std::env::var("COINGECKO_API_KEY").ok(),
        })
    }
}

fn required(key: &str) -> anyhow::Result<String> {
    std::env::var(key).with_context(|| format!("missing required env var: {key}"))
}
