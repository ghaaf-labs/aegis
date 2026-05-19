//! Live smoke test for the Pyth Hermes API.
//!
//! `#[ignore]` by default — run manually with `cargo test --ignored live_pyth`.

use aegis_api::modules::prices::{lookup_symbol, PriceProvider, PythProvider};

#[tokio::test]
#[ignore]
async fn pyth_returns_btc_in_plausible_band() {
    let http = reqwest::Client::builder()
        .user_agent("aegis-live-smoke/0.1.0")
        .build()
        .unwrap();
    let provider = PythProvider::new(http);
    let symbols: Vec<&_> = ["BTC", "ETH"]
        .iter()
        .filter_map(|t| lookup_symbol(t))
        .collect();
    let quotes = provider.fetch_spot(&symbols).await.expect("pyth fetch ok");
    let btc = quotes
        .iter()
        .find(|q| q.ticker == "BTC")
        .expect("btc returned");
    assert!(
        btc.price_usd > 10_000.0 && btc.price_usd < 500_000.0,
        "btc price out of band: {}",
        btc.price_usd
    );
}
