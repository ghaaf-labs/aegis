//! Route rule engine — the single place that decides whether a leg can really
//! execute. Consulted (via the same `validate_legs`) by the approval gate, the
//! executor's `ExecutionTicket::mint`, and (via `route_state_for_token` /
//! `executable_token_symbols`) by the agent and the UI. Fails closed: anything
//! missing a feature, address, adapter, or signer produces a blocker.

use crate::config::Config;

use super::super::models::{ChainKey, LegKind, PlannedLeg, TokenClass};
use super::capabilities::{AdapterCapability, RuntimeCapabilities};
use super::tokens::{self, EURC, USDC, USYC};

/// Plain-language route state surfaced to the UI for a token. One-to-one with
/// the user-facing labels: Ready, Track only, Needs route, Needs quote,
/// Needs address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RouteState {
    Ready,
    TrackOnly,
    NeedsRoute,
    NeedsQuote,
    NeedsAddress,
}

/// Stable machine codes for each way a route can fail closed. `wire_code()`
/// is the string surfaced to the frontend (kept compatible with the prior
/// `MissingCapability.code` values where they existed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockerCode {
    NonExecutionChain,
    UnknownToken,
    InvalidNotional,
    RealCctpFeature,
    UsdcAddress,
    CrossChainTokenSwap,
    RealSwapFeature,
    LocalSwapAdapter,
    SwapTokenAddress,
    UsycDisabled,
    RealUsycFeature,
    UsycAddress,
    StablefxUnavailable,
    MissingSigner,
}

impl BlockerCode {
    pub fn wire_code(self) -> &'static str {
        match self {
            BlockerCode::NonExecutionChain => "NON_EXECUTION_CHAIN",
            BlockerCode::UnknownToken => "UNKNOWN_TOKEN",
            BlockerCode::InvalidNotional => "INVALID_NOTIONAL",
            BlockerCode::RealCctpFeature => "REAL_CCTP_FEATURE",
            BlockerCode::UsdcAddress => "USDC_ADDRESS",
            BlockerCode::CrossChainTokenSwap => "CROSS_CHAIN_TOKEN_SWAP",
            BlockerCode::RealSwapFeature => "REAL_SWAP_FEATURE",
            BlockerCode::LocalSwapAdapter => "LOCAL_SWAP_ADAPTER",
            BlockerCode::SwapTokenAddress => "SWAP_TOKEN_ADDRESS",
            BlockerCode::UsycDisabled => "USYC_DISABLED",
            BlockerCode::RealUsycFeature => "REAL_USYC_FEATURE",
            BlockerCode::UsycAddress => "USYC_ADDRESS",
            BlockerCode::StablefxUnavailable => "STABLEFX_ADAPTER",
            BlockerCode::MissingSigner => "MISSING_SIGNER",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BlockerCode::NonExecutionChain => "Route cannot execute yet",
            BlockerCode::UnknownToken => "Unknown token",
            BlockerCode::InvalidNotional => "Invalid amount",
            BlockerCode::RealCctpFeature => "Bridge rail not enabled",
            BlockerCode::UsdcAddress => "Needs USDC address",
            BlockerCode::CrossChainTokenSwap => "Token buy route not ready",
            BlockerCode::RealSwapFeature => "Swap rail not enabled",
            BlockerCode::LocalSwapAdapter => "Swap route not ready",
            BlockerCode::SwapTokenAddress => "Needs token address",
            BlockerCode::UsycDisabled => "USYC is turned off",
            BlockerCode::RealUsycFeature => "USYC rail not enabled",
            BlockerCode::UsycAddress => "Needs USYC address",
            BlockerCode::StablefxUnavailable => "EURC route not ready",
            BlockerCode::MissingSigner => "Signer not configured",
        }
    }
}

/// One reason a plan cannot execute, with a user-facing detail string.
#[derive(Debug, Clone)]
pub struct RouteBlocker {
    pub code: BlockerCode,
    pub detail: String,
}

impl RouteBlocker {
    fn new(code: BlockerCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

/// Normalized leg used by the rule engine — works for planner output and for
/// stored DB rows. Chains stay as raw strings so wallet-only chains
/// (e.g. "eth-sepolia") can be flagged as non-execution rather than silently dropped.
#[derive(Debug, Clone)]
pub struct RouteLeg {
    pub kind: LegKind,
    pub src_chain: Option<String>,
    pub dest_chain: Option<String>,
    pub src_symbol: Option<String>,
    pub dest_symbol: Option<String>,
    pub amount_usdc: f64,
}

impl RouteLeg {
    pub fn from_planned(p: &PlannedLeg) -> Self {
        Self {
            kind: p.kind,
            src_chain: p.src_chain.map(|c| c.as_str().to_string()),
            dest_chain: p.dest_chain.map(|c| c.as_str().to_string()),
            src_symbol: p.src_symbol.clone(),
            dest_symbol: p.dest_symbol.clone(),
            amount_usdc: p.amount_usdc,
        }
    }

    pub fn from_parts(
        kind: &str,
        src_chain: Option<String>,
        dest_chain: Option<String>,
        src_symbol: Option<String>,
        dest_symbol: Option<String>,
        amount_usdc: f64,
    ) -> Option<Self> {
        Some(Self {
            kind: LegKind::parse(kind)?,
            src_chain,
            dest_chain,
            src_symbol,
            dest_symbol,
            amount_usdc,
        })
    }
}

/// The single rule engine. Returns the set of reasons the plan cannot execute
/// in real mode. Empty ⇒ every leg is executable. In mock mode (opt-in test/CI)
/// this returns empty because mock adapters never move real money.
pub fn validate_legs(
    caps: &RuntimeCapabilities,
    cfg: &Config,
    legs: &[RouteLeg],
) -> Vec<RouteBlocker> {
    if !caps.real_mode {
        return Vec::new();
    }

    let mut blockers: Vec<RouteBlocker> = Vec::new();

    // Non-execution chains — wallet support is not execution support. A chain
    // that parses to a known `ChainKey` but is not yet wired for execution
    // (no deployed RebalanceExecutor / swap venue) must still fail closed here,
    // so we gate on `is_execution()` rather than mere parseability.
    let mut non_exec: Vec<String> = legs
        .iter()
        .flat_map(|l| [l.src_chain.clone(), l.dest_chain.clone()])
        .flatten()
        .filter(|c| !ChainKey::parse(c).is_some_and(|k| k.is_execution()))
        .collect();
    non_exec.sort();
    non_exec.dedup();
    if !non_exec.is_empty() {
        blockers.push(RouteBlocker::new(
            BlockerCode::NonExecutionChain,
            format!(
                "Wallets are live on {}, but live rebalances execute only on Arc testnet and Base Sepolia.",
                non_exec.join(", ")
            ),
        ));
    }

    for leg in legs {
        if !leg.amount_usdc.is_finite() || leg.amount_usdc <= 0.0 {
            blockers.push(RouteBlocker::new(
                BlockerCode::InvalidNotional,
                "A leg has a non-positive or non-finite USDC amount.",
            ));
        }
        // Every leg kind needs at least one Arc/Base execution chain. A leg with
        // both chains unset, unparsable, or pointing only at a non-execution
        // chain must fail closed here, not late at submit time.
        let has_exec_chain = leg
            .src_chain
            .as_deref()
            .and_then(ChainKey::parse)
            .is_some_and(|k| k.is_execution())
            || leg
                .dest_chain
                .as_deref()
                .and_then(ChainKey::parse)
                .is_some_and(|k| k.is_execution());
        if !has_exec_chain {
            blockers.push(RouteBlocker::new(
                BlockerCode::NonExecutionChain,
                "Leg has no Arc or Base execution chain set.",
            ));
        }
        for sym in [leg.src_symbol.as_deref(), leg.dest_symbol.as_deref()]
            .into_iter()
            .flatten()
        {
            if tokens::token(sym).is_none() {
                blockers.push(RouteBlocker::new(
                    BlockerCode::UnknownToken,
                    format!("{sym} is not a known token."),
                ));
            }
        }

        match leg.kind {
            LegKind::CrossChainBurn => {
                push_cctp_blocker(caps, &mut blockers);
                // A non-USDC destination means a cross-chain hook swap: the
                // destination RebalanceExecutor swaps the minted USDC into the
                // target token (and refunds USDC on failure). The CCTP blocker
                // above already requires the destination RebalanceExecutor
                // address; the remaining requirement is that the target token
                // has a configured destination ERC-20 the contract can swap
                // into. Without it the contract has nothing to swap to, so fail
                // closed here.
                if let Some(sym) = leg
                    .dest_symbol
                    .as_deref()
                    .filter(|s| !s.eq_ignore_ascii_case(USDC))
                {
                    let dest_chain = leg
                        .dest_chain
                        .as_deref()
                        .and_then(ChainKey::parse)
                        .filter(|c| c.is_execution());
                    let has_dest_erc20 = dest_chain.is_some_and(|c| {
                        tokens::token(sym).is_some_and(|t| t.address_for(cfg, c).is_some())
                    });
                    if !has_dest_erc20 {
                        blockers.push(RouteBlocker::new(
                            BlockerCode::CrossChainTokenSwap,
                            format!(
                                "{sym} has no configured destination ERC-20, so the cross-chain hook swap cannot route. Remove that target sleeve for an executable review."
                            ),
                        ));
                    }
                }
            }
            LegKind::CrossChainMint => push_cctp_blocker(caps, &mut blockers),
            LegKind::LocalSwap => push_swap_blocker(caps, cfg, leg, &mut blockers),
            LegKind::ParkUsyc | LegKind::RedeemUsyc => push_usyc_blocker(caps, &mut blockers),
            LegKind::FxStablefx => blockers.push(RouteBlocker::new(
                BlockerCode::StablefxUnavailable,
                "EURC can be tracked as a target, but Arc StableFX is KYB-gated with no public testnet route. Remove the EURC sleeve for an executable review.",
            )),
        }
    }

    dedup_by_code(blockers)
}

fn push_cctp_blocker(caps: &RuntimeCapabilities, out: &mut Vec<RouteBlocker>) {
    let blocker = match caps.cctp {
        AdapterCapability::Live => return,
        AdapterCapability::NeedsFeature => RouteBlocker::new(
            BlockerCode::RealCctpFeature,
            "Restart the API with the real-cctp feature, then build a fresh review.",
        ),
        AdapterCapability::NeedsAddress => RouteBlocker::new(
            BlockerCode::UsdcAddress,
            "USDC token address is unset on Arc or Base; configure it before bridging.",
        ),
        AdapterCapability::NeedsSigner => RouteBlocker::new(
            BlockerCode::MissingSigner,
            "Chain signer (private key) is missing for Arc or Base.",
        ),
        AdapterCapability::Disabled | AdapterCapability::Unavailable(_) => {
            RouteBlocker::new(BlockerCode::RealCctpFeature, "CCTP bridge is unavailable.")
        }
    };
    out.push(blocker);
}

fn push_swap_blocker(
    caps: &RuntimeCapabilities,
    cfg: &Config,
    leg: &RouteLeg,
    out: &mut Vec<RouteBlocker>,
) {
    match caps.swap {
        AdapterCapability::NeedsFeature => out.push(RouteBlocker::new(
            BlockerCode::RealSwapFeature,
            "Restart the API with the real-swap feature to enable on-chain swaps.",
        )),
        AdapterCapability::NeedsAddress => out.push(RouteBlocker::new(
            BlockerCode::LocalSwapAdapter,
            "The swap venue (Uniswap V3 quoter/router on Base) is not configured.",
        )),
        AdapterCapability::NeedsSigner => out.push(RouteBlocker::new(
            BlockerCode::MissingSigner,
            "Base chain signer (private key) is missing.",
        )),
        AdapterCapability::Disabled | AdapterCapability::Unavailable(_) => out.push(
            RouteBlocker::new(BlockerCode::LocalSwapAdapter, "Swap route is unavailable."),
        ),
        AdapterCapability::Live => {
            // Venue is live — the specific token still needs a Base ERC-20.
            let symbol = swap_token_symbol(leg);
            let has_addr = symbol
                .and_then(tokens::token)
                .and_then(|t| t.address_for(cfg, ChainKey::Base))
                .is_some();
            if !has_addr {
                out.push(RouteBlocker::new(
                    BlockerCode::SwapTokenAddress,
                    format!(
                        "{} has no configured Base ERC-20 with a swap pool, so it can only be tracked.",
                        symbol.unwrap_or("This token")
                    ),
                ));
            }
        }
    }
}

fn push_usyc_blocker(caps: &RuntimeCapabilities, out: &mut Vec<RouteBlocker>) {
    let blocker = match caps.usyc {
        AdapterCapability::Live => return,
        AdapterCapability::Disabled => RouteBlocker::new(
            BlockerCode::UsycDisabled,
            "USYC is turned off: the Hashnote Teller on Arc is allowlist/KYB-gated. It can be tracked but not parked into.",
        ),
        AdapterCapability::NeedsFeature => RouteBlocker::new(
            BlockerCode::RealUsycFeature,
            "Restart the API with the real-usyc feature to enable USYC.",
        ),
        AdapterCapability::NeedsAddress => RouteBlocker::new(
            BlockerCode::UsycAddress,
            "USYC token or Teller address is unset on Arc.",
        ),
        AdapterCapability::NeedsSigner => RouteBlocker::new(
            BlockerCode::MissingSigner,
            "Arc chain signer (private key) is missing.",
        ),
        AdapterCapability::Unavailable(reason) => RouteBlocker::new(BlockerCode::UsycDisabled, reason),
    };
    out.push(blocker);
}

/// The non-USDC symbol involved in a swap leg (the one that needs a Base ERC-20).
fn swap_token_symbol(leg: &RouteLeg) -> Option<&str> {
    match leg.dest_symbol.as_deref() {
        Some(s) if !s.eq_ignore_ascii_case(USDC) => Some(s),
        _ => leg
            .src_symbol
            .as_deref()
            .filter(|s| !s.eq_ignore_ascii_case(USDC)),
    }
}

fn dedup_by_code(mut blockers: Vec<RouteBlocker>) -> Vec<RouteBlocker> {
    let mut seen: Vec<BlockerCode> = Vec::new();
    blockers.retain(|b| {
        if seen.contains(&b.code) {
            false
        } else {
            seen.push(b.code);
            true
        }
    });
    blockers
}

/// The plain-language route state for a token, for wallet/onboarding UI.
pub fn route_state_for_token(caps: &RuntimeCapabilities, cfg: &Config, symbol: &str) -> RouteState {
    let Some(spec) = tokens::token(symbol) else {
        return RouteState::TrackOnly;
    };
    match spec.class {
        // USDC is the settlement unit — always holdable/transferable.
        TokenClass::Stable => RouteState::Ready,
        TokenClass::Yield => cap_to_state(caps.usyc, true),
        TokenClass::FxStable => cap_to_state(caps.stablefx, true),
        TokenClass::Volatile => {
            let has_addr = spec.address_for(cfg, ChainKey::Base).is_some();
            cap_to_state(caps.swap, has_addr)
        }
    }
}

fn cap_to_state(cap: AdapterCapability, has_addr: bool) -> RouteState {
    match cap {
        AdapterCapability::Live if has_addr => RouteState::Ready,
        AdapterCapability::Live => RouteState::NeedsAddress,
        AdapterCapability::NeedsAddress if !has_addr => RouteState::NeedsAddress,
        AdapterCapability::NeedsFeature
        | AdapterCapability::NeedsSigner
        | AdapterCapability::NeedsAddress => RouteState::NeedsRoute,
        AdapterCapability::Disabled | AdapterCapability::Unavailable(_) => RouteState::TrackOnly,
    }
}

/// Symbols the agent may actually move funds into. Always USDC; USYC/EURC and
/// each volatile only when their adapter is live and (for volatiles) the token
/// has a configured Base ERC-20.
pub fn executable_token_symbols(caps: &RuntimeCapabilities, cfg: &Config) -> Vec<&'static str> {
    let mut out = vec![USDC];
    if caps.usyc.is_live() {
        out.push(USYC);
    }
    if caps.stablefx.is_live() {
        out.push(EURC);
    }
    if caps.swap.is_live() {
        for spec in tokens::TOKEN_REGISTRY {
            if spec.class == TokenClass::Volatile && spec.address_for(cfg, ChainKey::Base).is_some()
            {
                out.push(spec.symbol);
            }
        }
    }
    out
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

    fn leg(kind: LegKind, src: &str, dest: &str, dest_sym: &str) -> RouteLeg {
        RouteLeg {
            kind,
            src_chain: Some(src.into()),
            dest_chain: Some(dest.into()),
            src_symbol: Some("USDC".into()),
            dest_symbol: Some(dest_sym.into()),
            amount_usdc: 40.0,
        }
    }

    #[test]
    fn mock_mode_permits_everything() {
        let caps = RuntimeCapabilities::from_config(&crate::config::test_config());
        let legs = vec![leg(LegKind::FxStablefx, "arc", "arc", "EURC")];
        assert!(validate_legs(&caps, &crate::config::test_config(), &legs).is_empty());
    }

    #[test]
    fn usyc_is_blocked_disabled_in_real_mode() {
        let cfg = real_cfg();
        let caps = RuntimeCapabilities::from_config(&cfg);
        let legs = vec![leg(LegKind::ParkUsyc, "arc", "arc", "USYC")];
        let blockers = validate_legs(&caps, &cfg, &legs);
        assert!(blockers.iter().any(|b| b.code == BlockerCode::UsycDisabled));
    }

    #[test]
    fn stablefx_is_blocked_in_real_mode() {
        let cfg = real_cfg();
        let caps = RuntimeCapabilities::from_config(&cfg);
        let legs = vec![leg(LegKind::FxStablefx, "arc", "arc", "EURC")];
        let blockers = validate_legs(&caps, &cfg, &legs);
        assert!(blockers
            .iter()
            .any(|b| b.code == BlockerCode::StablefxUnavailable));
    }

    #[test]
    fn leg_without_any_execution_chain_is_blocked() {
        let cfg = real_cfg();
        let caps = RuntimeCapabilities::from_config(&cfg);
        let mut l = leg(LegKind::CrossChainBurn, "arc", "base", "USDC");
        l.src_chain = None;
        l.dest_chain = None;
        let blockers = validate_legs(&caps, &cfg, &[l]);
        assert!(blockers
            .iter()
            .any(|b| b.code == BlockerCode::NonExecutionChain));
    }

    #[test]
    fn non_execution_chain_is_flagged() {
        let cfg = real_cfg();
        let caps = RuntimeCapabilities::from_config(&cfg);
        let legs = vec![leg(LegKind::CrossChainBurn, "eth-sepolia", "base", "USDC")];
        let blockers = validate_legs(&caps, &cfg, &legs);
        assert!(blockers
            .iter()
            .any(|b| b.code == BlockerCode::NonExecutionChain));
    }

    #[test]
    fn usdc_is_always_executable_symbol() {
        let cfg = real_cfg();
        let caps = RuntimeCapabilities::from_config(&cfg);
        assert!(executable_token_symbols(&caps, &cfg).contains(&"USDC"));
        // USYC disabled, EURC KYB-gated → never executable here.
        assert!(!executable_token_symbols(&caps, &cfg).contains(&"USYC"));
        assert!(!executable_token_symbols(&caps, &cfg).contains(&"EURC"));
    }

    #[test]
    fn cross_chain_hook_swap_blocked_without_dest_erc20() {
        // ETH has no configured Base ERC-20 in this cfg → the hook swap cannot
        // route, so the CrossChainTokenSwap blocker fires.
        let cfg = real_cfg();
        let caps = RuntimeCapabilities::from_config(&cfg);
        let legs = vec![leg(LegKind::CrossChainBurn, "arc", "base", "ETH")];
        let blockers = validate_legs(&caps, &cfg, &legs);
        assert!(blockers
            .iter()
            .any(|b| b.code == BlockerCode::CrossChainTokenSwap));
    }

    #[test]
    fn cross_chain_hook_swap_allowed_with_dest_erc20() {
        // With a configured Base ERC-20 for the target token, the dedicated
        // CrossChainTokenSwap blocker no longer fires (CCTP feature gating is a
        // separate blocker and is allowed to remain).
        let mut cfg = real_cfg();
        cfg.weth_base = "0x4200000000000000000000000000000000000006".into();
        let caps = RuntimeCapabilities::from_config(&cfg);
        let legs = vec![leg(LegKind::CrossChainBurn, "arc", "base", "ETH")];
        let blockers = validate_legs(&caps, &cfg, &legs);
        assert!(!blockers
            .iter()
            .any(|b| b.code == BlockerCode::CrossChainTokenSwap));
    }

    #[test]
    fn usyc_token_is_track_only_when_disabled() {
        let cfg = real_cfg();
        let caps = RuntimeCapabilities::from_config(&cfg);
        assert_eq!(
            route_state_for_token(&caps, &cfg, "USYC"),
            RouteState::TrackOnly
        );
        assert_eq!(
            route_state_for_token(&caps, &cfg, "EURC"),
            RouteState::TrackOnly
        );
        assert_eq!(
            route_state_for_token(&caps, &cfg, "USDC"),
            RouteState::Ready
        );
    }
}
