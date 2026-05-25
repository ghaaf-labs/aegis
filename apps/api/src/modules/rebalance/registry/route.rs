//! Route rule engine — the single place that decides whether a leg can really
//! execute. Consulted (via the same `validate_legs`) by the approval gate, the
//! executor's `ExecutionTicket::mint`, and (via `route_state_for_token` /
//! `executable_token_symbols`) by the agent and the UI. Fails closed: anything
//! missing a feature, address, adapter, or signer produces a blocker.

use crate::config::Config;

use super::super::models::{ChainKey, LegKind, PlannedLeg, TokenClass};
use super::capabilities::{AdapterCapability, RuntimeCapabilities};
use super::tokens::{self, USDC, USYC};

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
                "Wallets are live on {}, but that chain is not wired for live rebalance execution yet.",
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
                // A non-USDC destination means a cross-chain hook swap: the
                // destination RebalanceExecutor swaps the minted USDC into the
                // target token (and refunds USDC on failure), so the route must
                // also have the destination executor wired.
                let hooked = leg
                    .dest_symbol
                    .as_deref()
                    .is_some_and(|s| !s.eq_ignore_ascii_case(USDC));
                push_cctp_blocker(cfg, leg, hooked, &mut blockers);
                // For a hook swap the remaining requirement is that the target
                // token has a configured destination ERC-20 the contract can
                // swap into. Without it the contract has nothing to swap to, so
                // fail closed here.
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
            LegKind::CrossChainMint => push_cctp_blocker(cfg, leg, false, &mut blockers),
            LegKind::LocalSwap => push_swap_blocker(cfg, leg, &mut blockers),
            LegKind::ParkUsyc | LegKind::RedeemUsyc => push_usyc_blocker(caps, &mut blockers),
            LegKind::FxStablefx => blockers.push(RouteBlocker::new(
                BlockerCode::StablefxUnavailable,
                "EURC can be tracked as a target, but Arc StableFX is KYB-gated with no public testnet route. Remove the EURC sleeve for an executable review.",
            )),
        }
    }

    dedup_by_code(blockers)
}

/// Validate a CCTP burn/mint leg against the *specific* chains it touches, not
/// an Arc/Base aggregate — so a bridge from a funded-but-unwired wallet chain
/// (e.g. ETH-Sepolia) fails closed at approval rather than after the source
/// burn. `hooked` is true when the destination needs the executor's hook swap.
fn push_cctp_blocker(cfg: &Config, leg: &RouteLeg, hooked: bool, out: &mut Vec<RouteBlocker>) {
    let (Some(src), Some(dest)) = (
        leg.src_chain.as_deref().and_then(ChainKey::parse),
        leg.dest_chain.as_deref().and_then(ChainKey::parse),
    ) else {
        // A missing/unparsable chain is already reported by the
        // NonExecutionChain checks above; nothing route-specific to add.
        return;
    };
    let blocker = match crate::modules::rebalance::adapters::cctp::capability_for_route(
        cfg, src, dest, hooked,
    ) {
        AdapterCapability::Live => return,
        AdapterCapability::NeedsFeature => RouteBlocker::new(
            BlockerCode::RealCctpFeature,
            "Restart the API with the real-cctp feature, then build a fresh review.",
        ),
        AdapterCapability::NeedsAddress => RouteBlocker::new(
            BlockerCode::UsdcAddress,
            format!(
                "CCTP rail is not fully configured for {} → {} (USDC / messenger / transmitter{}).",
                src.as_str(),
                dest.as_str(),
                if hooked { " / executor" } else { "" },
            ),
        ),
        AdapterCapability::NeedsSigner => RouteBlocker::new(
            BlockerCode::MissingSigner,
            format!(
                "Chain signer (private key) is missing for {} or {}.",
                src.as_str(),
                dest.as_str(),
            ),
        ),
        AdapterCapability::Disabled | AdapterCapability::Unavailable(_) => {
            RouteBlocker::new(BlockerCode::RealCctpFeature, "CCTP bridge is unavailable.")
        }
    };
    out.push(blocker);
}

fn push_swap_blocker(cfg: &Config, leg: &RouteLeg, out: &mut Vec<RouteBlocker>) {
    // A swap is same-chain; resolve the leg's execution chain (fall back to Base,
    // the chain whose venue is wired today, for a chain-less leg) and validate
    // *that* chain's venue rather than the Base aggregate, so a swap on a chain
    // with no configured router/quoter fails closed at approval.
    let chain = swap_leg_chain(leg).unwrap_or(ChainKey::Base);
    match crate::modules::rebalance::adapters::swap::capability_for(cfg, chain) {
        AdapterCapability::NeedsFeature => out.push(RouteBlocker::new(
            BlockerCode::RealSwapFeature,
            "Restart the API with the real-swap feature to enable on-chain swaps.",
        )),
        AdapterCapability::NeedsAddress => out.push(RouteBlocker::new(
            BlockerCode::LocalSwapAdapter,
            format!(
                "The swap venue (router/quoter) is not configured on {}.",
                chain.as_str(),
            ),
        )),
        AdapterCapability::NeedsSigner => out.push(RouteBlocker::new(
            BlockerCode::MissingSigner,
            format!(
                "Chain signer (private key) is missing for {}.",
                chain.as_str()
            ),
        )),
        AdapterCapability::Disabled | AdapterCapability::Unavailable(_) => out.push(
            RouteBlocker::new(BlockerCode::LocalSwapAdapter, "Swap route is unavailable."),
        ),
        AdapterCapability::Live => {
            // Venue is live — the specific token still needs an ERC-20 on the
            // swap leg's chain.
            let symbol = swap_token_symbol(leg);
            let has_addr = symbol
                .and_then(tokens::token)
                .and_then(|t| t.address_for(cfg, chain))
                .is_some();
            if !has_addr {
                out.push(RouteBlocker::new(
                    BlockerCode::SwapTokenAddress,
                    format!(
                        "{} has no configured {} ERC-20 with a swap pool, so it can only be tracked.",
                        symbol.unwrap_or("This token"),
                        chain.as_str(),
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

/// The execution chain a swap leg settles on (a swap is same-chain). Takes the
/// destination, falling back to source; `None` if neither parses to a wired
/// execution chain.
fn swap_leg_chain(leg: &RouteLeg) -> Option<ChainKey> {
    leg.dest_chain
        .as_deref()
        .or(leg.src_chain.as_deref())
        .and_then(ChainKey::parse)
        .filter(|c| c.is_execution())
}

/// The non-USDC symbol involved in a swap leg (the one that needs an ERC-20).
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
        // EURC (the only FxStable) trades on the permissionless USDC/EURC pool on
        // Base, so it routes through the swap adapter exactly like a volatile —
        // the gated Arc StableFX rail (`caps.stablefx`) is superseded.
        TokenClass::FxStable | TokenClass::Volatile => {
            let has_addr = spec.address_for(cfg, ChainKey::Base).is_some();
            // An ERC-20 is configured but the deployment's liquidity allowlist
            // says there's no tradeable pool here (e.g. EURC/LINK/cbBTC on Base
            // Sepolia) → honest track-only, never an execution target that would
            // revert at gas-estimation.
            if has_addr && !cfg.swap_token_has_venue(symbol, ChainKey::Base) {
                return RouteState::TrackOnly;
            }
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

/// Symbols the agent may actually move funds into. Always USDC; USYC only when
/// its adapter is live; every swap-acquired token (volatiles + EURC, which now
/// trades on the Base USDC/EURC pool) only when the swap adapter is live and the
/// token has a configured Base ERC-20.
pub fn executable_token_symbols(caps: &RuntimeCapabilities, cfg: &Config) -> Vec<&'static str> {
    let mut out = vec![USDC];
    if caps.usyc.is_live() {
        out.push(USYC);
    }
    if caps.swap.is_live() {
        for spec in tokens::TOKEN_REGISTRY {
            let swap_acquired = matches!(spec.class, TokenClass::Volatile | TokenClass::FxStable);
            if swap_acquired
                && spec.address_for(cfg, ChainKey::Base).is_some()
                && cfg.swap_token_has_venue(spec.symbol, ChainKey::Base)
            {
                out.push(spec.symbol);
            }
        }
    }
    out
}

/// Symbols the allocator may place in a target allocation.
///
/// This is intentionally the product's designable sleeve menu, not the current
/// executable-only subset. Execution readiness is surfaced separately through
/// [`route_state_for_token`] / [`executable_token_symbols`]. Collapsing the
/// allocator universe to only "ready right now" made real-mode accounts fall
/// back to a 100% USDC target whenever swap rails were not fully live, which
/// produced a misleading no-op instead of an actionable review.
pub fn allocation_target_symbols(cfg: &Config) -> Vec<&'static str> {
    designable_allocation_symbols(cfg)
}

/// The product's supported sleeve universe, derived from each token's
/// [`TokenSpec::designable`] flag.
///
/// USYC is the one runtime-gated sleeve: it is only offered while `USYC_ENABLED`
/// (declared via its [`TokenSpec::gate`]), otherwise it stays coming-soon /
/// context-only and must not appear as an investable target.
pub fn designable_allocation_symbols(cfg: &Config) -> Vec<&'static str> {
    tokens::TOKEN_REGISTRY
        .iter()
        .filter(|spec| spec.is_designable_target(cfg))
        .map(|spec| spec.symbol)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn designable_universe_is_independent_of_swap_liveness() {
        let cfg = crate::config::test_config();
        let designable = designable_allocation_symbols(&cfg);
        // The product sleeve menu is present regardless of adapter *liveness*
        // (cargo features / funded signers) — but only routable-in-principle
        // sleeves are offered (no un-routable L1-natives; see the registry).
        for sym in [
            USDC,
            tokens::CBBTC,
            tokens::ETH,
            tokens::EURC,
            tokens::CBETH,
            "LINK",
            "UNI",
            "AERO",
        ] {
            assert!(designable.contains(&sym), "designable should include {sym}");
        }
        // In a mock/offline build the executable set collapses to USDC, but the
        // designable universe must NOT — conflating the two was the bug.
        let caps = RuntimeCapabilities::from_config(&cfg);
        let executable = executable_token_symbols(&caps, &cfg);
        assert!(!executable.contains(&tokens::CBBTC));
        assert!(designable.contains(&tokens::CBBTC));
        assert!(designable.len() > executable.len());
    }

    #[test]
    fn designable_excludes_price_only_and_unroutable_tokens() {
        let designable = designable_allocation_symbols(&crate::config::test_config());
        // Price-only `BTC` (cbBTC is the sleeve), wrong-chain `WBTC`, the
        // misclassified `sUSDS`, and the un-routable other-L1-natives (no
        // Arc/Base venue) must not be offered — the agent can only design what we
        // can route. (LINK/UNI/AERO are now designable; see the other test.)
        for sym in [
            "BTC",
            tokens::WBTC,
            tokens::SUSDS,
            "SOL",
            "BNB",
            "AVAX",
            "MATIC",
        ] {
            assert!(!designable.contains(&sym), "designable must exclude {sym}");
        }
    }

    #[test]
    fn designable_gates_usyc_on_the_usyc_enabled_flag() {
        let mut cfg = crate::config::test_config();
        cfg.usyc_enabled = false;
        assert!(!designable_allocation_symbols(&cfg).contains(&USYC));
        cfg.usyc_enabled = true;
        assert!(designable_allocation_symbols(&cfg).contains(&USYC));
    }

    #[test]
    fn allocation_targets_keep_designable_sleeves_in_mock_mode() {
        let cfg = crate::config::test_config();
        let targets = allocation_target_symbols(&cfg);
        assert!(targets.contains(&USDC));
        assert!(targets.contains(&"UNI"));
        assert!(targets.contains(&tokens::CBBTC));
    }

    fn real_cfg() -> Config {
        let mut cfg = crate::config::test_config();
        cfg.execution_mock = false;
        cfg.circle_mock = false;
        cfg.chains[ChainKey::Arc.index()].private_key = "0xaa".into();
        cfg.chains[ChainKey::Base.index()].private_key = "0xbb".into();
        cfg.chains[ChainKey::Arc.index()].usdc =
            "0x00000000000000000000000000000000000000a1".into();
        cfg.chains[ChainKey::Base.index()].usdc =
            "0x036CbD53842c5426634e7929541eC2318f3dCF7e".into();
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
        // All six EVM testnets are execution chains; an unparsable / non-EVM
        // chain must still fail closed with a NonExecutionChain blocker.
        let cfg = real_cfg();
        let caps = RuntimeCapabilities::from_config(&cfg);
        let legs = vec![leg(LegKind::CrossChainBurn, "solana", "base", "USDC")];
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
        // USYC disabled → never executable here. EURC needs the swap adapter
        // live + a Base ERC-20; neither is set in this cfg, so not executable.
        assert!(!executable_token_symbols(&caps, &cfg).contains(&"USYC"));
        assert!(!executable_token_symbols(&caps, &cfg).contains(&"EURC"));
    }

    #[test]
    fn allocation_targets_stay_designable_in_real_mode() {
        let cfg = real_cfg();
        let caps = RuntimeCapabilities::from_config(&cfg);
        let targets = allocation_target_symbols(&cfg);
        let executable = executable_token_symbols(&caps, &cfg);
        assert!(targets.contains(&USDC));
        assert!(targets.contains(&"UNI"));
        assert!(!executable.contains(&"UNI"));
        assert!(targets.len() > executable.len());
    }

    #[test]
    fn eurc_is_executable_when_swap_live_with_base_erc20() {
        // EURC now executes on the Base USDC/EURC DEX pool. When the swap
        // adapter is live and EURC has a configured Base ERC-20, it joins the
        // executable set (no longer gated behind StableFX).
        let mut cfg = real_cfg();
        cfg.set_token_address(
            "EURC",
            ChainKey::Base,
            "0x60a3E35Cc302bFA44Cb288Bc5a4F316Fdb1adb42",
        );
        cfg.swap_liquid_tokens
            .insert(ChainKey::Base, vec!["EURC".into()]);
        let mut caps = RuntimeCapabilities::from_config(&cfg);
        caps.swap = AdapterCapability::Live;
        assert!(executable_token_symbols(&caps, &cfg).contains(&"EURC"));
        assert_eq!(
            route_state_for_token(&caps, &cfg, "EURC"),
            RouteState::Ready
        );
    }

    #[test]
    fn unset_liquidity_allowlist_uses_safe_testnet_default() {
        let mut cfg = real_cfg();
        cfg.set_token_address(
            "ETH",
            ChainKey::Base,
            "0x4200000000000000000000000000000000000006",
        );
        cfg.set_token_address(
            "cbBTC",
            ChainKey::Base,
            "0xcbb7c0006f23900c38eb856149f799620fcb8a4a",
        );
        cfg.set_token_address(
            "LINK",
            ChainKey::Base,
            "0xE4aB69C077896252FAFBD49EFD26B5D171A32410",
        );
        let mut caps = RuntimeCapabilities::from_config(&cfg);
        caps.swap = AdapterCapability::Live;

        let executable = executable_token_symbols(&caps, &cfg);
        assert!(executable.contains(&tokens::ETH));
        assert!(!executable.contains(&tokens::CBBTC));
        assert!(!executable.contains(&"LINK"));
    }

    #[test]
    fn liquid_venue_allowlist_curates_executable_set() {
        // A token with a configured Base ERC-20 + a live swap rail is NOT
        // executable when the deployment's liquidity allowlist excludes it
        // (e.g. only WETH/USDC has a real pool on Base Sepolia). The agent and
        // planner both read this, so no swap leg is ever built for cbBTC/EURC.
        let mut cfg = real_cfg();
        cfg.set_token_address(
            "ETH",
            ChainKey::Base,
            "0x4200000000000000000000000000000000000006",
        );
        cfg.set_token_address(
            "cbBTC",
            ChainKey::Base,
            "0xcbb7c0006f23900c38eb856149f799620fcb8a4a",
        );
        cfg.set_token_address(
            "EURC",
            ChainKey::Base,
            "0x808456652fdb597867f38412077A9182bf77359F",
        );
        cfg.swap_liquid_tokens
            .insert(ChainKey::Base, vec!["ETH".into()]);
        let mut caps = RuntimeCapabilities::from_config(&cfg);
        caps.swap = AdapterCapability::Live;

        let executable = executable_token_symbols(&caps, &cfg);
        assert!(executable.contains(&"USDC"));
        assert!(executable.contains(&tokens::ETH));
        assert!(!executable.contains(&tokens::CBBTC));
        assert!(!executable.contains(&"EURC"));
        assert_eq!(
            route_state_for_token(&caps, &cfg, tokens::ETH),
            RouteState::Ready
        );
        assert_eq!(
            route_state_for_token(&caps, &cfg, tokens::CBBTC),
            RouteState::TrackOnly
        );
        assert_eq!(
            route_state_for_token(&caps, &cfg, "EURC"),
            RouteState::TrackOnly
        );
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
        cfg.set_token_address(
            "ETH",
            ChainKey::Base,
            "0x4200000000000000000000000000000000000006",
        );
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
        // EURC now routes via the swap adapter; without the real-swap feature
        // the venue reports NeedsFeature, so EURC needs a route (not the old
        // track-only StableFX state) — matching the volatile sleeves.
        assert_eq!(
            route_state_for_token(&caps, &cfg, "EURC"),
            route_state_for_token(&caps, &cfg, "cbBTC"),
        );
        assert_eq!(
            route_state_for_token(&caps, &cfg, "USDC"),
            RouteState::Ready
        );
    }
}
