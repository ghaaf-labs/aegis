//! model_bench — rank OpenRouter models for the allocator task.
//!
//! Renders the *real* Allocator prompt for a few representative portfolio cases,
//! calls each candidate model `--runs` times, and reports latency p50/p95,
//! parse-error rate, constraint-validity rate (weights sum ~100, ≤60% single
//! non-stable cap, designable-only), and mean $/call. Use it to choose the
//! `MODEL_STRATEGIST` fallback chain (lead with the fastest reliable model).
//!
//! Usage:
//!   cargo run --bin model_bench -- \
//!     --models "deepseek/deepseek-v4-flash,~google/gemini-flash-latest,~anthropic/claude-sonnet-latest" \
//!     --runs 3
//!
//! Needs OPENROUTER_API_KEY (real calls; costs a few cents). Not a CI test —
//! each candidate is benchmarked alone (single-element chain → no fallback) so
//! the numbers reflect that model, not the chain.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;

use aegis_api::config::{Config, ModelRoute};
use aegis_api::env;
use aegis_api::modules::ai::{
    strip_json_fences, Message, OpenRouterClient, PromptKey, PromptRegistry,
};
use aegis_api::modules::rebalance::registry::designable_allocation_symbols;

/// Stablecoin sleeves exempt from the single-asset cap (they may sit well above
/// 60%). Mirrors the allocator's stable classifier closely enough for scoring.
const STABLE: &[&str] = &["USDC", "EURC", "USYC", "sUSDS"];

struct CliArgs {
    models: Vec<String>,
    runs: u32,
}

fn parse_args() -> anyhow::Result<CliArgs> {
    let mut models: Vec<String> = Vec::new();
    let mut runs: u32 = 3;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--models" => {
                models = it
                    .next()
                    .context("--models requires a comma-separated list")?
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "--runs" => {
                runs = it
                    .next()
                    .context("--runs requires a value")?
                    .parse()
                    .context("--runs must be a positive integer")?;
            }
            "-h" | "--help" => {
                eprintln!(
                    "Usage: model_bench [--models a,b,c] [--runs N]\n  --models  OpenRouter slugs to benchmark (default: the current MODEL_STRATEGIST chain)\n  --runs    Calls per (model, case) (default 3)"
                );
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
    }
    Ok(CliArgs { models, runs })
}

/// One benchmark portfolio scenario — a populated Allocator context.
struct Case {
    name: &'static str,
    ctx: HashMap<&'static str, String>,
}

/// Representative context shared by every case; cases override the dials below.
fn base_ctx(route_caps: &str) -> HashMap<&'static str, String> {
    let mut c: HashMap<&'static str, String> = HashMap::new();
    for (k, v) in [
        ("objective", "grow steadily while limiting drawdowns"),
        ("risk_tolerance", "moderate"),
        ("horizon_months", "24"),
        ("total_value_usd", "10000"),
        (
            "goal_block",
            "Goal: build a diversified crypto position with a stablecoin core.",
        ),
        ("allocations_table", "(empty — first deploy)"),
        ("wallet_block", "Gateway USDC: 10000 (undeployed)"),
        ("memory", "(no prior decisions)"),
        ("regime", "neutral"),
        ("regime_confidence", "0.7"),
        ("btc_vol_30d", "0.45"),
        ("corr_90d", "0.6"),
        ("max_drawdown", "0.25"),
        ("fear_greed", "50"),
        ("btc_dominance", "54"),
        ("concentration_risk", "low"),
        ("volatility_score", "medium"),
        ("drift_score", "0"),
        ("usyc_rate", "4.8% (Track-only)"),
        ("usdc_eurc_basis", "1.08"),
        ("harvestable_losses", "(none)"),
    ] {
        c.insert(k, v.to_string());
    }
    c.insert("route_capabilities", route_caps.to_string());
    c
}

fn cases(route_caps: &str) -> Vec<Case> {
    let mut out = Vec::new();

    let mut conservative = base_ctx(route_caps);
    conservative.insert("risk_tolerance", "conservative".into());
    conservative.insert("horizon_months", "6".into());
    conservative.insert("regime", "risk_off".into());
    conservative.insert("btc_vol_30d", "0.75".into());
    conservative.insert("fear_greed", "22".into());
    conservative.insert(
        "objective",
        "preserve capital with a small growth sleeve".into(),
    );
    out.push(Case {
        name: "conservative/risk_off",
        ctx: conservative,
    });

    let mut aggressive = base_ctx(route_caps);
    aggressive.insert("risk_tolerance", "aggressive".into());
    aggressive.insert("horizon_months", "48".into());
    aggressive.insert("regime", "risk_on".into());
    aggressive.insert("btc_vol_30d", "0.35".into());
    aggressive.insert("fear_greed", "74".into());
    aggressive.insert("objective", "maximize long-term growth".into());
    out.push(Case {
        name: "aggressive/risk_on",
        ctx: aggressive,
    });

    out.push(Case {
        name: "moderate/neutral",
        ctx: base_ctx(route_caps),
    });

    out
}

#[derive(Default)]
struct Stat {
    latencies: Vec<u64>,
    parse_ok: u32,
    valid_ok: u32,
    calls: u32,
    errors: u32,
    cost_sum: f64,
    cost_n: u32,
}

enum Outcome {
    Unparsable,
    Parsed { valid: bool },
}

/// Parse the allocator output and check the deterministic constraints the real
/// `finalize_allocation` enforces: parseable JSON, weights sum ≈ 100, every key
/// in the designable universe, and no single non-stable sleeve above 60%.
fn parse_and_validate(content: &str, designable: &[&str]) -> Outcome {
    let stripped = strip_json_fences(content);
    let Ok(value) = serde_json::from_str::<serde_json::Value>(stripped) else {
        return Outcome::Unparsable;
    };
    let Some(alloc) = value
        .get("recommendedAllocation")
        .and_then(|v| v.as_object())
    else {
        return Outcome::Unparsable;
    };
    if alloc.is_empty() {
        return Outcome::Parsed { valid: false };
    }

    let mut sum = 0.0_f64;
    let mut max_non_stable = 0.0_f64;
    let mut all_designable = true;
    for (sym, weight) in alloc {
        let w = weight.as_f64().unwrap_or(-1.0);
        if w < 0.0 {
            return Outcome::Parsed { valid: false };
        }
        sum += w;
        if !designable.contains(&sym.as_str()) {
            all_designable = false;
        }
        if !STABLE.contains(&sym.as_str()) {
            max_non_stable = max_non_stable.max(w);
        }
    }

    let valid = (98.0..=102.0).contains(&sum) && all_designable && max_non_stable <= 60.0;
    Outcome::Parsed { valid }
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env::load_env();
    let args = parse_args()?;
    let mut cfg = Config::from_env()?;
    // Default to the configured strategist chain when --models is omitted.
    let models = if args.models.is_empty() {
        cfg.models_for(ModelRoute::RebalanceReason)
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    } else {
        args.models.clone()
    };

    let http = reqwest::Client::builder()
        .user_agent(concat!("Aegis-ModelBench/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(180))
        .build()?;
    let prompts = PromptRegistry::load().await;
    let designable = designable_allocation_symbols(&cfg);
    let route_caps = format!(
        "Designable universe (you MAY allocate to any of these): {}\nExecutable now: USDC. Held until rail live: the rest (still valid targets — never reroute them to USDC).",
        designable.join(", ")
    );
    let cases = cases(&route_caps);

    eprintln!(
        "model_bench: {} model(s) × {} case(s) × {} run(s) = {} calls\n",
        models.len(),
        cases.len(),
        args.runs,
        models.len() * cases.len() * args.runs as usize
    );

    let mut results: Vec<(String, Stat)> = Vec::new();
    for model in &models {
        // Benchmark this model alone (single-element chain → no fallback).
        cfg.model_strategist = model.clone();
        let ai = OpenRouterClient::new(&http, &cfg);
        let mut stat = Stat::default();
        for case in &cases {
            let prompt = prompts.render(PromptKey::Allocator, &case.ctx);
            for _ in 0..args.runs {
                stat.calls += 1;
                let messages = vec![
                    Message::system(prompt.clone()),
                    Message::user("Design the target allocation as JSON.".to_string()),
                ];
                match ai.chat(ModelRoute::RebalanceReason, messages).await {
                    Ok(resp) => {
                        stat.latencies.push(resp.latency_ms);
                        if let Some(c) = resp.cost_usd {
                            stat.cost_sum += c;
                            stat.cost_n += 1;
                        }
                        match parse_and_validate(&resp.content, &designable) {
                            Outcome::Parsed { valid } => {
                                stat.parse_ok += 1;
                                if valid {
                                    stat.valid_ok += 1;
                                }
                            }
                            Outcome::Unparsable => {}
                        }
                    }
                    Err(e) => {
                        stat.errors += 1;
                        eprintln!("  [{model}] {} call error: {e}", case.name);
                    }
                }
            }
        }
        results.push((model.clone(), stat));
    }

    // Rank: highest valid rate, then lowest p50 latency.
    results.sort_by(|a, b| {
        let av = a.1.valid_ok as f64 / a.1.calls.max(1) as f64;
        let bv = b.1.valid_ok as f64 / b.1.calls.max(1) as f64;
        let mut a_lat = a.1.latencies.clone();
        let mut b_lat = b.1.latencies.clone();
        a_lat.sort_unstable();
        b_lat.sort_unstable();
        bv.partial_cmp(&av)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(percentile(&a_lat, 50.0).cmp(&percentile(&b_lat, 50.0)))
    });

    println!("\n## model_bench results (ranked: valid% desc, then p50 asc)\n");
    println!("| model | calls | parse% | valid% | p50 ms | p95 ms | mean $ | err |");
    println!("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
    for (model, s) in &results {
        let mut lat = s.latencies.clone();
        lat.sort_unstable();
        let calls = s.calls.max(1) as f64;
        let mean_cost = if s.cost_n > 0 {
            s.cost_sum / s.cost_n as f64
        } else {
            0.0
        };
        println!(
            "| {} | {} | {:.0}% | {:.0}% | {} | {} | {:.4} | {} |",
            model,
            s.calls,
            100.0 * s.parse_ok as f64 / calls,
            100.0 * s.valid_ok as f64 / calls,
            percentile(&lat, 50.0),
            percentile(&lat, 95.0),
            mean_cost,
            s.errors,
        );
    }
    println!(
        "\nPick the fastest model with valid% at/near 100 as the MODEL_STRATEGIST primary;\nkeep the others as the cross-vendor fallback chain.\n"
    );
    Ok(())
}
