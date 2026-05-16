use std::sync::atomic::{AtomicU64, Ordering};

static AGENT_DECISIONS: AtomicU64 = AtomicU64::new(0);
static REBALANCES_SUCCEEDED: AtomicU64 = AtomicU64::new(0);
static REBALANCES_FAILED: AtomicU64 = AtomicU64::new(0);
static USDC_MOVED_CENTS: AtomicU64 = AtomicU64::new(0);

pub fn record_agent_decision() {
    AGENT_DECISIONS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_rebalance_succeeded(usdc_moved: f64) {
    REBALANCES_SUCCEEDED.fetch_add(1, Ordering::Relaxed);
    if let Some(cents) = usdc_cents(usdc_moved) {
        USDC_MOVED_CENTS.fetch_add(cents, Ordering::Relaxed);
    }
}

fn usdc_cents(amount: f64) -> Option<u64> {
    if amount.is_finite() && amount > 0.0 {
        Some((amount * 100.0).round() as u64)
    } else {
        None
    }
}

pub fn record_rebalance_failed() {
    REBALANCES_FAILED.fetch_add(1, Ordering::Relaxed);
}

pub fn render_prometheus() -> String {
    let agent = AGENT_DECISIONS.load(Ordering::Relaxed);
    let ok = REBALANCES_SUCCEEDED.load(Ordering::Relaxed);
    let fail = REBALANCES_FAILED.load(Ordering::Relaxed);
    let moved = USDC_MOVED_CENTS.load(Ordering::Relaxed);
    format!(
        "# HELP aegis_agent_decisions_total Agent decisions persisted.\n\
         # TYPE aegis_agent_decisions_total counter\n\
         aegis_agent_decisions_total {agent}\n\
         # HELP aegis_rebalances_succeeded_total Rebalances reaching status=completed.\n\
         # TYPE aegis_rebalances_succeeded_total counter\n\
         aegis_rebalances_succeeded_total {ok}\n\
         # HELP aegis_rebalances_failed_total Rebalances reaching status=failed.\n\
         # TYPE aegis_rebalances_failed_total counter\n\
         aegis_rebalances_failed_total {fail}\n\
         # HELP aegis_usdc_moved_cents_total Sum of USDC moved by completed rebalances, in cents.\n\
         # TYPE aegis_usdc_moved_cents_total counter\n\
         aegis_usdc_moved_cents_total {moved}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_contains_all_four_counters() {
        let out = render_prometheus();
        assert!(out.contains("aegis_agent_decisions_total"));
        assert!(out.contains("aegis_rebalances_succeeded_total"));
        assert!(out.contains("aegis_rebalances_failed_total"));
        assert!(out.contains("aegis_usdc_moved_cents_total"));
    }

    #[test]
    fn usdc_cents_converts_dollars_to_cents() {
        assert_eq!(usdc_cents(1.0), Some(100));
        assert_eq!(usdc_cents(12.34), Some(1234));
        assert_eq!(usdc_cents(0.01), Some(1));
    }

    #[test]
    fn usdc_cents_rejects_non_positive_or_non_finite() {
        assert_eq!(usdc_cents(0.0), None);
        assert_eq!(usdc_cents(-1.0), None);
        assert_eq!(usdc_cents(f64::NAN), None);
        assert_eq!(usdc_cents(f64::INFINITY), None);
    }
}
