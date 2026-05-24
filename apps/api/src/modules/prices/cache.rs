use chrono::Utc;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::provider::{PriceProvider, SpotQuote, Symbol};

/// In-process per-ticker cache TTL. Picked at 3s so two consumers firing in
/// quick succession (peg monitor at 10s, sse ticker at 5s) can share one
/// upstream call when their timing aligns, but no quote ever serves for more
/// than a few seconds.
const CACHE_TTL: Duration = Duration::from_secs(3);

/// Circuit-breaker thresholds. After this many consecutive failures the
/// primary is skipped for `OPEN_SECS` before being retried.
const FAILURE_THRESHOLD: u32 = 3;
const OPEN_SECS: i64 = 60;

/// Wraps a primary + fallback provider with caching and a circuit breaker.
/// Implements `PriceProvider` itself so consumers see one trait object.
pub struct FallbackProvider {
    primary: Arc<dyn PriceProvider>,
    fallback: Arc<dyn PriceProvider>,
    cache: Mutex<HashMap<&'static str, (Instant, SpotQuote)>>,
    failures: AtomicU32,
    open_until_unix: AtomicI64,
    /// `true` when the last successful fetch came from the fallback (not
    /// the primary). `name()` reads this so the SSE `source` field is
    /// honest: a single primary failure flips the next call to fallback
    /// even though the circuit isn't open yet, and the user-facing
    /// "via X · live tick" should reflect that.
    last_was_fallback: AtomicBool,
}

impl FallbackProvider {
    pub fn new(primary: Arc<dyn PriceProvider>, fallback: Arc<dyn PriceProvider>) -> Self {
        Self {
            primary,
            fallback,
            cache: Mutex::new(HashMap::new()),
            failures: AtomicU32::new(0),
            open_until_unix: AtomicI64::new(0),
            last_was_fallback: AtomicBool::new(false),
        }
    }

    fn circuit_open(&self) -> bool {
        // Acquire so a freshly written breaker timestamp from another thread
        // is observed promptly. Relaxed was OK on x86 but risked stale reads
        // on weakly-ordered architectures (ARM Graviton, Apple Silicon).
        let until = self.open_until_unix.load(Ordering::Acquire);
        until > 0 && Utc::now().timestamp() < until
    }

    fn record_success(&self) {
        self.failures.store(0, Ordering::Relaxed);
        self.open_until_unix.store(0, Ordering::Release);
    }

    fn record_failure(&self) {
        let n = self.failures.fetch_add(1, Ordering::Relaxed) + 1;
        if n >= FAILURE_THRESHOLD {
            self.open_until_unix
                .store(Utc::now().timestamp() + OPEN_SECS, Ordering::Release);
        }
    }

    fn read_cache(&self, symbols: &[&Symbol]) -> (Vec<SpotQuote>, Vec<&'static Symbol>) {
        let now = Instant::now();
        let mut hits = Vec::new();
        let mut misses = Vec::new();
        let cache = self.cache.lock().unwrap();
        for s in symbols {
            match cache.get(s.symbol) {
                Some((at, q)) if now.duration_since(*at) < CACHE_TTL => hits.push(q.clone()),
                _ => {
                    // Re-resolve to a 'static reference via the symbol table —
                    // a fresh request never carries borrowed lifetimes past
                    // the cache.
                    if let Some(sym) = super::lookup_symbol(s.symbol) {
                        misses.push(sym);
                    }
                }
            }
        }
        (hits, misses)
    }

    fn write_cache(&self, quotes: &[SpotQuote]) {
        let mut cache = self.cache.lock().unwrap();
        let now = Instant::now();
        for q in quotes {
            cache.insert(q.ticker, (now, q.clone()));
        }
    }
}

#[async_trait::async_trait]
impl PriceProvider for FallbackProvider {
    async fn fetch_spot(&self, symbols: &[&Symbol]) -> anyhow::Result<Vec<SpotQuote>> {
        let (mut out, misses) = self.read_cache(symbols);
        if misses.is_empty() {
            return Ok(out);
        }

        let miss_refs: Vec<&Symbol> = misses.to_vec();
        let active = if self.circuit_open() {
            &self.fallback
        } else {
            &self.primary
        };

        let active_is_fallback = std::ptr::eq(
            Arc::as_ptr(active) as *const (),
            Arc::as_ptr(&self.fallback) as *const (),
        );
        match active.fetch_spot(&miss_refs).await {
            Ok(quotes) => {
                if !active_is_fallback {
                    self.record_success();
                }
                self.last_was_fallback
                    .store(active_is_fallback, Ordering::Relaxed);
                self.write_cache(&quotes);
                out.extend(quotes);
                Ok(out)
            }
            Err(primary_err) => {
                // Primary fail → record, try fallback once before giving up.
                if !active_is_fallback {
                    self.record_failure();
                    tracing::warn!(error = %primary_err, "primary price provider failed; trying fallback");
                    let quotes = self.fallback.fetch_spot(&miss_refs).await?;
                    self.last_was_fallback.store(true, Ordering::Relaxed);
                    self.write_cache(&quotes);
                    out.extend(quotes);
                    Ok(out)
                } else {
                    Err(primary_err)
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        // Source reported to consumers (SSE `source` field, ProvenanceLine,
        // `price_history.source` column) must reflect whichever provider
        // *actually served the last successful fetch*. A single primary
        // failure that flipped to fallback still flips name(), even before
        // the circuit breaker opens.
        if self.last_was_fallback.load(Ordering::Relaxed) || self.circuit_open() {
            self.fallback.name()
        } else {
            self.primary.name()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::prices::lookup_symbol;

    struct FixedProvider {
        name: &'static str,
        quote: f64,
    }

    #[async_trait::async_trait]
    impl PriceProvider for FixedProvider {
        async fn fetch_spot(&self, symbols: &[&Symbol]) -> anyhow::Result<Vec<SpotQuote>> {
            Ok(symbols
                .iter()
                .map(|s| SpotQuote {
                    ticker: s.symbol,
                    price_usd: self.quote,
                    change_24h: 0.0,
                    change_7d: 0.0,
                    market_cap: 0.0,
                    volume_24h: 0.0,
                    observed_at: Utc::now(),
                    confidence: None,
                })
                .collect())
        }
        fn name(&self) -> &'static str {
            self.name
        }
    }

    struct FailingProvider {
        name: &'static str,
    }

    #[async_trait::async_trait]
    impl PriceProvider for FailingProvider {
        async fn fetch_spot(&self, _: &[&Symbol]) -> anyhow::Result<Vec<SpotQuote>> {
            anyhow::bail!("simulated upstream failure")
        }
        fn name(&self) -> &'static str {
            self.name
        }
    }

    #[tokio::test]
    async fn returns_primary_when_healthy() {
        let fb = FallbackProvider::new(
            Arc::new(FixedProvider {
                name: "p",
                quote: 100.0,
            }),
            Arc::new(FixedProvider {
                name: "f",
                quote: 200.0,
            }),
        );
        let btc = lookup_symbol("BTC").unwrap();
        let out = fb.fetch_spot(&[btc]).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].price_usd, 100.0);
    }

    #[tokio::test]
    async fn falls_back_when_primary_errors() {
        let fb = FallbackProvider::new(
            Arc::new(FailingProvider { name: "p" }),
            Arc::new(FixedProvider {
                name: "f",
                quote: 200.0,
            }),
        );
        let btc = lookup_symbol("BTC").unwrap();
        let out = fb.fetch_spot(&[btc]).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].price_usd, 200.0);
    }

    #[tokio::test]
    async fn cache_short_circuits_repeat_calls() {
        let fb = FallbackProvider::new(
            Arc::new(FixedProvider {
                name: "p",
                quote: 100.0,
            }),
            Arc::new(FixedProvider {
                name: "f",
                quote: 200.0,
            }),
        );
        let btc = lookup_symbol("BTC").unwrap();
        let first = fb.fetch_spot(&[btc]).await.unwrap();
        let second = fb.fetch_spot(&[btc]).await.unwrap();
        // Same ticker, second call hits cache — value identical.
        assert_eq!(first[0].price_usd, second[0].price_usd);
    }

    #[tokio::test]
    async fn circuit_opens_after_threshold_failures() {
        let fb = FallbackProvider::new(
            Arc::new(FailingProvider { name: "p" }),
            Arc::new(FixedProvider {
                name: "f",
                quote: 200.0,
            }),
        );
        let btc = lookup_symbol("BTC").unwrap();
        // Each call records a failure on the primary; after 3 the breaker opens
        // and subsequent calls go straight to fallback without primary attempt.
        for _ in 0..FAILURE_THRESHOLD {
            let _ = fb.fetch_spot(&[btc]).await;
            // Bust the cache between iterations so each call actually attempts the primary.
            fb.cache.lock().unwrap().clear();
        }
        assert!(fb.circuit_open());
        assert_eq!(fb.name(), "f");
    }
}
