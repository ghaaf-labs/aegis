//! `ExecutionTicket` — the un-forgeable authorization to execute one real leg.
//!
//! The struct has a private `_seal` field, so the *only* way to obtain one is
//! `ExecutionTicket::mint`, which runs the full route rule engine and quote
//! validation. Real adapter methods take `&ExecutionTicket`, so a real
//! execution path cannot be reached without passing every gate — there is no
//! way to fall through to a synthetic/mock hash by construction.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::config::Config;

use super::super::models::{ChainKey, LegKind};
use super::super::quote::{self, QuoteError, QuoteExpectation, ValidatedQuote};
use super::capabilities::RuntimeCapabilities;
use super::route::{self, RouteBlocker, RouteLeg};

#[derive(Debug)]
pub enum MintError {
    /// One or more route capabilities are missing (fail closed).
    Blocked(Vec<RouteBlocker>),
    /// The supplied quote is stale, mismatched, or unsafe.
    Quote(QuoteError),
    /// The leg's chains could not be resolved to execution chains.
    BadChain,
}

impl MintError {
    pub fn detail(&self) -> String {
        match self {
            MintError::Blocked(bs) => bs
                .iter()
                .map(|b| b.detail.clone())
                .collect::<Vec<_>>()
                .join("; "),
            MintError::Quote(q) => q.detail().to_string(),
            MintError::BadChain => "leg chains are not valid execution chains".to_string(),
        }
    }
}

/// A validated, ready-to-execute leg. Construct only via `mint`.
#[derive(Debug, Clone)]
pub struct ExecutionTicket {
    leg_id: Uuid,
    kind: LegKind,
    src_chain: ChainKey,
    dest_chain: ChainKey,
    src_symbol: String,
    dest_symbol: String,
    amount_usdc: f64,
    quote: ValidatedQuote,
    _seal: (),
}

impl ExecutionTicket {
    /// The single constructor. Returns `Err` unless the leg clears every route
    /// capability check and the quote is fresh and self-consistent.
    pub fn mint(
        caps: &RuntimeCapabilities,
        cfg: &Config,
        leg_id: Uuid,
        leg: &RouteLeg,
        quote: ValidatedQuote,
        now: DateTime<Utc>,
    ) -> Result<Self, MintError> {
        let blockers = route::validate_legs(caps, cfg, std::slice::from_ref(leg));
        if !blockers.is_empty() {
            return Err(MintError::Blocked(blockers));
        }

        let src_chain = leg
            .src_chain
            .as_deref()
            .or(leg.dest_chain.as_deref())
            .and_then(ChainKey::parse)
            .ok_or(MintError::BadChain)?;
        let dest_chain = leg
            .dest_chain
            .as_deref()
            .or(leg.src_chain.as_deref())
            .and_then(ChainKey::parse)
            .ok_or(MintError::BadChain)?;

        let src_symbol = leg.src_symbol.clone().unwrap_or_else(|| "USDC".into());
        let dest_symbol = leg.dest_symbol.clone().unwrap_or_else(|| "USDC".into());

        // A CCTP burn/mint quote is the 1:1 USDC bridge. A *hooked* burn carries
        // its swap target (ETH/cbBTC/…) in `dest_symbol` for the destination
        // RebalanceExecutor, but the bridge quote itself is always USDC→USDC —
        // the USDC→token swap is the hook, validated by its own `min_out`. So
        // validate CCTP legs against the bridged unit; local swaps against the
        // real token pair. (`dest_symbol` is still stored below for the hook.)
        let (expect_src, expect_dest) = match leg.kind {
            LegKind::CrossChainBurn | LegKind::CrossChainMint => ("USDC", "USDC"),
            _ => (src_symbol.as_str(), dest_symbol.as_str()),
        };

        quote::validate(
            &quote,
            QuoteExpectation {
                src_token: expect_src,
                dest_token: expect_dest,
                src_chain,
                dest_chain,
            },
            now,
        )
        .map_err(MintError::Quote)?;

        Ok(Self {
            leg_id,
            kind: leg.kind,
            src_chain,
            dest_chain,
            src_symbol,
            dest_symbol,
            amount_usdc: leg.amount_usdc,
            quote,
            _seal: (),
        })
    }

    pub fn leg_id(&self) -> Uuid {
        self.leg_id
    }
    pub fn kind(&self) -> LegKind {
        self.kind
    }
    pub fn src_chain(&self) -> ChainKey {
        self.src_chain
    }
    pub fn dest_chain(&self) -> ChainKey {
        self.dest_chain
    }
    pub fn src_symbol(&self) -> &str {
        &self.src_symbol
    }
    pub fn dest_symbol(&self) -> &str {
        &self.dest_symbol
    }
    pub fn amount_usdc(&self) -> f64 {
        self.amount_usdc
    }
    pub fn quote(&self) -> &ValidatedQuote {
        &self.quote
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real_cfg() -> Config {
        let mut cfg = crate::config::test_config();
        cfg.execution_mock = false;
        cfg.circle_mock = false;
        cfg.chain_private_key_arc = "0xaa".into();
        cfg.chain_private_key_base = "0xbb".into();
        cfg.usdc_arc = "0x00000000000000000000000000000000000000a1".into();
        cfg.usdc_base = "0x036CbD53842c5426634e7929541eC2318f3dCF7e".into();
        cfg
    }

    fn usyc_leg() -> RouteLeg {
        RouteLeg {
            kind: LegKind::ParkUsyc,
            src_chain: Some("arc".into()),
            dest_chain: Some("arc".into()),
            src_symbol: Some("USDC".into()),
            dest_symbol: Some("USYC".into()),
            amount_usdc: 40.0,
        }
    }

    #[test]
    fn mint_fails_closed_for_disabled_usyc() {
        let cfg = real_cfg();
        let caps = RuntimeCapabilities::from_config(&cfg);
        let q =
            ValidatedQuote::cctp_one_to_one(ChainKey::Arc, ChainKey::Arc, 40_000_000, Utc::now());
        let err = ExecutionTicket::mint(&caps, &cfg, Uuid::new_v4(), &usyc_leg(), q, Utc::now())
            .unwrap_err();
        assert!(matches!(err, MintError::Blocked(_)));
    }

    #[test]
    fn mint_rejects_stale_quote_in_mock_mode_bridge() {
        // Mock mode passes route checks, so a stale quote is the gate that fires.
        let cfg = crate::config::test_config();
        let caps = RuntimeCapabilities::from_config(&cfg);
        let leg = RouteLeg {
            kind: LegKind::CrossChainBurn,
            src_chain: Some("arc".into()),
            dest_chain: Some("base".into()),
            src_symbol: Some("USDC".into()),
            dest_symbol: Some("USDC".into()),
            amount_usdc: 40.0,
        };
        let now = Utc::now();
        let stale = ValidatedQuote::cctp_one_to_one(
            ChainKey::Arc,
            ChainKey::Base,
            40_000_000,
            now - chrono::Duration::seconds(120),
        );
        let err = ExecutionTicket::mint(&caps, &cfg, Uuid::new_v4(), &leg, stale, now).unwrap_err();
        assert!(matches!(err, MintError::Quote(QuoteError::Expired)));
    }

    #[test]
    fn mint_accepts_hooked_burn_against_usdc_bridge_quote() {
        // A hooked CrossChainBurn carries its swap target (ETH) in dest_symbol,
        // but the CCTP bridge quote is USDC->USDC. Minting must validate the
        // quote against the bridged unit, not ETH (regression for the real-exec
        // failure "quote token does not match the leg"). dest_symbol is still
        // preserved on the ticket for the destination hook.
        let cfg = crate::config::test_config();
        let caps = RuntimeCapabilities::from_config(&cfg);
        let leg = RouteLeg {
            kind: LegKind::CrossChainBurn,
            src_chain: Some("arc".into()),
            dest_chain: Some("base".into()),
            src_symbol: Some("USDC".into()),
            dest_symbol: Some("ETH".into()),
            amount_usdc: 4.0,
        };
        let now = Utc::now();
        let q = ValidatedQuote::cctp_one_to_one(ChainKey::Arc, ChainKey::Base, 4_000_000, now);
        let ticket =
            ExecutionTicket::mint(&caps, &cfg, Uuid::new_v4(), &leg, q, now).expect("hooked burn");
        assert_eq!(ticket.dest_symbol, "ETH");
    }
}
