//! The settlement-leg state machine (spec §8) as a *data-driven* transition
//! table, plus the funds-location model that the fund-safety theorem (§17) is
//! checked against.
//!
//! Encoding the legal transitions as data — not a nest of `match` arms scattered
//! through the executor — lets us exhaustively model-check three safety
//! properties at test time (§15.2 / §18):
//!   1. terminals have no outgoing transitions;
//!   2. every non-terminal can reach a terminal (no stuck states);
//!   3. **fund-safety**: every terminal leaves funds as the target asset, as
//!      USDC, or unmoved — never stranded in flight or in an intermediate token.
//!
//! Honest cross-chain modelling: a CCTP burn is irreversible, so once funds are
//! `InFlight` (burned, not yet minted) the only progress is to `BridgeLanded`
//! (USDC on the destination) — an attestation timeout is a *retry*, never a
//! no-movement failure. That is why no terminal is ever `InFlight`.

/// State of a single settlement leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LegState {
    Pending,
    Quoted,
    Submitted,
    /// CCTP burn confirmed on the source; awaiting attestation + mint.
    BridgeInFlight,
    /// USDC minted on the destination; awaiting the acquire (swap) step.
    BridgeLanded,
    // ── terminals ──
    /// Target asset acquired (same-chain swap, or cross-chain bridge+acquire).
    Confirmed,
    /// Reverted before any funds moved — a clean halt.
    Failed,
    /// Bridged USDC landed but the acquire didn't run; funds rest as idle USDC
    /// (a recorded outcome the next plan can redeploy).
    StrandedReserve,
    /// The destination forwarder refunded USDC in-transaction after a failed
    /// dest swap; funds are back in the user's custody as USDC.
    CompensatedToUsdc,
}

/// What triggers a transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LegEvent {
    Quote,
    Submit,
    /// Same-chain swap confirmed.
    LocalConfirm,
    /// Tx reverted before funds moved (swap revert / pre-burn failure).
    RevertBeforeFunds,
    /// CCTP burn confirmed on the source chain.
    BurnConfirm,
    /// Attestation + mint landed on the destination.
    Mint,
    /// Attestation still pending — retry (burned funds will still mint).
    AttestTimeout,
    /// Destination acquire (swap) succeeded.
    DestAcquire,
    /// Destination acquire failed; minted USDC kept idle.
    DestAcquireFailed,
    /// Hooked-burn forwarder refunded USDC in the same destination tx.
    ForwarderRefunded,
}

/// Where a leg's funds physically are while in a given state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FundsLocation {
    /// Nothing has moved yet (or a clean revert returned to start).
    NotMoved,
    /// Burned on the source, not yet minted on the destination.
    InFlight,
    /// Sitting as USDC in the user's custody.
    Usdc,
    /// Held as the approved target asset.
    TargetAsset,
}

/// The legal transition table — the single source of truth for leg progression.
const TRANSITIONS: &[(LegState, LegEvent, LegState)] = &[
    (LegState::Pending, LegEvent::Quote, LegState::Quoted),
    (LegState::Quoted, LegEvent::Submit, LegState::Submitted),
    (
        LegState::Submitted,
        LegEvent::LocalConfirm,
        LegState::Confirmed,
    ),
    (
        LegState::Submitted,
        LegEvent::RevertBeforeFunds,
        LegState::Failed,
    ),
    (
        LegState::Submitted,
        LegEvent::BurnConfirm,
        LegState::BridgeInFlight,
    ),
    // Irreversible burn: a timeout retries; it can never lose the burned funds.
    (
        LegState::BridgeInFlight,
        LegEvent::AttestTimeout,
        LegState::BridgeInFlight,
    ),
    (
        LegState::BridgeInFlight,
        LegEvent::Mint,
        LegState::BridgeLanded,
    ),
    (
        LegState::BridgeLanded,
        LegEvent::DestAcquire,
        LegState::Confirmed,
    ),
    (
        LegState::BridgeLanded,
        LegEvent::DestAcquireFailed,
        LegState::StrandedReserve,
    ),
    (
        LegState::BridgeLanded,
        LegEvent::ForwarderRefunded,
        LegState::CompensatedToUsdc,
    ),
];

impl LegState {
    /// All variants — for exhaustive model-checking and DB round-tripping.
    pub const ALL: [LegState; 9] = [
        LegState::Pending,
        LegState::Quoted,
        LegState::Submitted,
        LegState::BridgeInFlight,
        LegState::BridgeLanded,
        LegState::Confirmed,
        LegState::Failed,
        LegState::StrandedReserve,
        LegState::CompensatedToUsdc,
    ];

    /// Apply `event`; returns the next state iff the transition is legal.
    pub fn on(self, event: LegEvent) -> Option<LegState> {
        TRANSITIONS
            .iter()
            .find(|(from, ev, _)| *from == self && *ev == event)
            .map(|(_, _, to)| *to)
    }

    /// A terminal state has no outgoing transitions — the leg is done.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            LegState::Confirmed
                | LegState::Failed
                | LegState::StrandedReserve
                | LegState::CompensatedToUsdc
        )
    }

    /// Where the leg's funds are while in this state.
    pub fn funds_location(self) -> FundsLocation {
        match self {
            LegState::Pending | LegState::Quoted | LegState::Submitted | LegState::Failed => {
                FundsLocation::NotMoved
            }
            LegState::BridgeInFlight => FundsLocation::InFlight,
            LegState::BridgeLanded | LegState::StrandedReserve | LegState::CompensatedToUsdc => {
                FundsLocation::Usdc
            }
            LegState::Confirmed => FundsLocation::TargetAsset,
        }
    }

    /// Stable string for persistence (matches the `rebalance_legs.leg_state`
    /// CHECK in the migration the executor adopts).
    pub fn as_str(self) -> &'static str {
        match self {
            LegState::Pending => "pending",
            LegState::Quoted => "quoted",
            LegState::Submitted => "submitted",
            LegState::BridgeInFlight => "bridge_in_flight",
            LegState::BridgeLanded => "bridge_landed",
            LegState::Confirmed => "confirmed",
            LegState::Failed => "failed",
            LegState::StrandedReserve => "stranded_reserve",
            LegState::CompensatedToUsdc => "compensated_to_usdc",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const EVENTS: [LegEvent; 10] = [
        LegEvent::Quote,
        LegEvent::Submit,
        LegEvent::LocalConfirm,
        LegEvent::RevertBeforeFunds,
        LegEvent::BurnConfirm,
        LegEvent::Mint,
        LegEvent::AttestTimeout,
        LegEvent::DestAcquire,
        LegEvent::DestAcquireFailed,
        LegEvent::ForwarderRefunded,
    ];

    #[test]
    fn terminals_have_no_outgoing_transitions() {
        for state in LegState::ALL {
            if state.is_terminal() {
                for ev in EVENTS {
                    assert_eq!(
                        state.on(ev),
                        None,
                        "terminal {state:?} must not transition on {ev:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn on_matches_the_table_and_rejects_illegal_events() {
        for state in LegState::ALL {
            for ev in EVENTS {
                let in_table = TRANSITIONS
                    .iter()
                    .any(|(from, e, _)| *from == state && *e == ev);
                assert_eq!(
                    state.on(ev).is_some(),
                    in_table,
                    "{state:?} on {ev:?}: `on` and the table must agree"
                );
            }
        }
    }

    #[test]
    fn every_non_terminal_can_reach_a_terminal() {
        // BFS over the transition graph from each non-terminal.
        for start in LegState::ALL {
            if start.is_terminal() {
                continue;
            }
            let mut seen = HashSet::new();
            let mut frontier = vec![start];
            let mut reached_terminal = false;
            while let Some(s) = frontier.pop() {
                if !seen.insert(s) {
                    continue;
                }
                if s.is_terminal() {
                    reached_terminal = true;
                    break;
                }
                for ev in EVENTS {
                    if let Some(next) = s.on(ev) {
                        frontier.push(next);
                    }
                }
            }
            assert!(
                reached_terminal,
                "non-terminal {start:?} must have a path to a terminal (no stuck states)"
            );
        }
    }

    #[test]
    fn fund_safety_theorem_every_terminal_is_target_usdc_or_unmoved() {
        // §17: a terminal leg never strands funds in flight or an intermediate
        // token — it ends as the target asset, as USDC, or with nothing moved.
        for state in LegState::ALL {
            if state.is_terminal() {
                assert!(
                    matches!(
                        state.funds_location(),
                        FundsLocation::TargetAsset | FundsLocation::Usdc | FundsLocation::NotMoved
                    ),
                    "FUND-SAFETY VIOLATION: terminal {state:?} leaves funds {:?}",
                    state.funds_location()
                );
            }
        }
    }

    #[test]
    fn in_flight_funds_are_never_in_a_terminal_state() {
        // The CCTP-irreversibility guarantee: burned-but-unminted funds only ever
        // exist in a non-terminal state that must progress to BridgeLanded.
        for state in LegState::ALL {
            if state.funds_location() == FundsLocation::InFlight {
                assert!(
                    !state.is_terminal(),
                    "{state:?} holds in-flight funds but is terminal — funds could be lost"
                );
            }
        }
    }

    #[test]
    fn table_transitions_only_originate_from_non_terminals() {
        for (from, _, _) in TRANSITIONS {
            assert!(
                !from.is_terminal(),
                "no transition may originate from terminal {from:?}"
            );
        }
    }
}
