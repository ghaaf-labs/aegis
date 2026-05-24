//! USYC adapter boundary — park/redeem via the Hashnote Teller on Arc.
//!
//! Currently gated off (`USYC_ENABLED=false`): the Arc testnet Teller is
//! allowlist/KYB-gated (deposits revert `0x7f63bd0f`) and Circle's CCTP docs
//! list USYC only on Ethereum/BNB. While disabled, USYC legs fail closed in
//! the route rule engine and never reach execution. This module reports the
//! capability so the registry, agent, and UI all agree.

use crate::config::Config;

use super::super::models::ChainKey;
use super::super::registry::capabilities::AdapterCapability;
use super::super::registry::tokens;

pub fn capability(cfg: &Config) -> AdapterCapability {
    if !cfg.usyc_enabled {
        AdapterCapability::Disabled
    } else if !cfg!(feature = "real-usyc") {
        AdapterCapability::NeedsFeature
    } else if !tokens::is_real_addr(&cfg.usyc_token_arc)
        || !tokens::is_real_addr(&cfg.usyc_teller_arc)
    {
        AdapterCapability::NeedsAddress
    } else if cfg.chain(ChainKey::Arc).private_key.trim().is_empty() {
        AdapterCapability::NeedsSigner
    } else {
        AdapterCapability::Live
    }
}
