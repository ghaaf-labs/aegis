//! Arc StableFX adapter boundary — native USDC↔EURC FX.
//!
//! Institutional/KYB-gated with no public self-serve testnet API, so it is
//! structurally unavailable for self-serve Aegis. EURC can be tracked as a
//! target but FX legs fail closed in the route rule engine. This module
//! reports the capability so the registry, agent, and UI agree.

use crate::config::Config;

use super::super::registry::capabilities::AdapterCapability;

pub fn capability(_cfg: &Config) -> AdapterCapability {
    AdapterCapability::Unavailable("Arc StableFX is KYB-gated; no public testnet route")
}
