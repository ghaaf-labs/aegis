//! F-REG-3 — CLI entry point for the regime-classifier backtest.
//!
//! Usage:
//!   cargo run --bin regime_backtest -- --years 5 --model anthropic/claude-haiku-4.5
//!
//! Reuses the same Config + Pool + OpenRouter client the API server uses.
//! On completion prints a summary and the persisted eval_run_id.

use std::time::Duration;

use anyhow::Context;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use aegis_api::config::Config;
use aegis_api::db;
use aegis_api::modules::ai::{OpenRouterClient, PromptRegistry};
use aegis_api::modules::risk_engine::regime_backtest::{run_backtest, OpenRouterRegimeClassifier};

#[derive(Debug)]
struct CliArgs {
    years: u32,
    model: Option<String>,
}

fn parse_args() -> anyhow::Result<CliArgs> {
    let mut years: u32 = 5;
    let mut model: Option<String> = None;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--years" => {
                years = it
                    .next()
                    .context("--years requires a value")?
                    .parse()
                    .context("--years must be a positive integer")?;
            }
            "--model" => {
                model = Some(it.next().context("--model requires a value")?);
            }
            "-h" | "--help" => {
                eprintln!(
                    "Usage: regime_backtest [--years N] [--model SLUG]\n  --years   Number of years of price_history to walk (default 5)\n  --model   OpenRouter slug to evaluate (default: $MODEL_REGIME)"
                );
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
    }
    Ok(CliArgs { years, model })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aegis_api=info,regime_backtest=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().compact())
        .init();

    let args = parse_args()?;
    let mut cfg = Config::from_env()?;
    if let Some(m) = &args.model {
        cfg.model_regime = m.clone();
    }

    let pool = db::connect(&cfg.database_url).await?;
    let http = reqwest::Client::builder()
        .user_agent(concat!("Aegis-RegimeBacktest/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(120))
        .build()?;
    let prompts = PromptRegistry::load().await;
    let ai = OpenRouterClient::new(&http, &cfg);

    let classifier = OpenRouterRegimeClassifier {
        ai,
        prompts: &prompts,
        // 4 calls/sec → 250ms min delay between calls. The 429-backoff path
        // inside the classifier doubles from 500ms upward.
        min_delay: Duration::from_millis(250),
        max_retries: 5,
    };

    info!(
        "starting regime backtest: years={} model={}",
        args.years, cfg.model_regime
    );
    let run = run_backtest(&pool, &cfg.model_regime, args.years, &classifier).await?;

    println!();
    println!("Regime backtest complete");
    println!("  eval_run_id      : {}", run.eval_run_id);
    println!("  model_slug       : {}", run.model_slug);
    println!(
        "  period           : {} → {}",
        run.period_start, run.period_end
    );
    println!("  samples          : {}", run.samples_count);
    println!("  accuracy         : {:.4}", run.accuracy);
    println!("  precision (macro): {:.4}", run.precision_macro);
    println!("  recall    (macro): {:.4}", run.recall_macro);
    println!("  F1        (macro): {:.4}", run.f1_macro);
    println!("  Brier score      : {:.4}", run.brier_score);
    println!("  confusion        : {:?}", run.confusion);

    Ok(())
}
