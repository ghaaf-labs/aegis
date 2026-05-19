//! Live smoke test for the DefiLlama Coins API.
//!
//! Hits `coins.llama.fi/prices/current` with no auth and asserts BTC and USDC
//! are present and in plausible bands. `#[ignore]` by default — run manually
//! with `cargo test --ignored live_defillama`.

use aegis_api::modules::prices::{lookup_symbol, DefiLlamaProvider, PriceProvider};

#[tokio::test]
#[ignore]
async fn defillama_returns_btc_and_usdc_in_plausible_bands() {
    let http = reqwest::Client::builder()
        .user_agent("aegis-live-smoke/0.1.0")
        .build()
        .unwrap();
    let provider = DefiLlamaProvider::new(http);
    let symbols: Vec<&_> = ["BTC", "USDC"]
        .iter()
        .filter_map(|t| lookup_symbol(t))
        .collect();
    let quotes = provider
        .fetch_spot(&symbols)
        .await
        .expect("defillama spot fetch ok");

    let btc = quotes
        .iter()
        .find(|q| q.ticker == "BTC")
        .expect("btc returned");
    assert!(
        btc.price_usd > 10_000.0 && btc.price_usd < 500_000.0,
        "btc price out of band: {}",
        btc.price_usd
    );

    let usdc = quotes
        .iter()
        .find(|q| q.ticker == "USDC")
        .expect("usdc returned");
    assert!(
        (usdc.price_usd - 1.0).abs() < 0.05,
        "usdc not near peg: {}",
        usdc.price_usd
    );
}
