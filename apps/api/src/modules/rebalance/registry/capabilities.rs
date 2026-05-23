//! Runtime capability snapshot — what can *actually* execute right now.
//!
//! Built once from `Config` + compile-time cargo features. Describes the
//! capability of each real adapter (CCTP bridge, per-chain swap, USYC,
//! StableFX) so the route rule engine can fail closed without re-deriving
//! these facts in four places.

use crate::config::Config;

use super::super::adapters;

/// Why an adapter is or isn't able to execute real transactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterCapability {
    /// Fully wired: feature compiled, addresses set, signer present.
    Live,
    /// The required cargo feature (`real-cctp` / `real-usyc` / `real-swap`)
    /// was not compiled in.
    NeedsFeature,
    /// A required contract/token address is unset or a zero placeholder.
    NeedsAddress,
    /// The chain signer (EOA private key) is missing in real mode.
    NeedsSigner,
    /// Turned off by an explicit kill-switch (USYC).
    Disabled,
    /// Structurally unavailable on testnet (e.g. KYB-gated StableFX).
    Unavailable(&'static str),
}

impl AdapterCapability {
    pub fn is_live(self) -> bool {
        matches!(self, AdapterCapability::Live)
    }
}

/// Snapshot of what real execution can do, derived from config + features.
#[derive(Debug, Clone)]
pub struct RuntimeCapabilities {
    /// Real mode = neither execution nor Circle is mocked. When false the
    /// app runs against opt-in mock adapters (tests/CI/offline dev) and the
    /// route rule engine permits everything (mock receipts, never real money).
    pub real_mode: bool,
    pub real_cctp_compiled: bool,
    pub real_usyc_compiled: bool,
    pub real_swap_compiled: bool,
    pub usyc_enabled: bool,
    pub signer_arc: bool,
    pub signer_base: bool,
    pub cctp: AdapterCapability,
    pub swap: AdapterCapability,
    pub usyc: AdapterCapability,
    pub stablefx: AdapterCapability,
}

impl RuntimeCapabilities {
    pub fn from_config(cfg: &Config) -> Self {
        // Each adapter owns the logic for its own capability; the registry just
        // composes the snapshot.
        Self {
            real_mode: !cfg.execution_mock && !cfg.circle_mock,
            real_cctp_compiled: cfg!(feature = "real-cctp"),
            real_usyc_compiled: cfg!(feature = "real-usyc"),
            real_swap_compiled: cfg!(feature = "real-swap"),
            usyc_enabled: cfg.usyc_enabled,
            signer_arc: !cfg.chain_private_key_arc.trim().is_empty(),
            signer_base: !cfg.chain_private_key_base.trim().is_empty(),
            cctp: adapters::cctp::capability(cfg),
            swap: adapters::swap::capability(cfg),
            usyc: adapters::usyc::capability(cfg),
            stablefx: adapters::stablefx::capability(cfg),
        }
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
        cfg.usdc_arc = "0x0000000000000000000000000000000000000abc".into();
        cfg.usdc_base = "0x036CbD53842c5426634e7929541eC2318f3dCF7e".into();
        cfg
    }

    #[test]
    fn mock_mode_is_detected() {
        let caps = RuntimeCapabilities::from_config(&crate::config::test_config());
        assert!(!caps.real_mode);
    }

    #[test]
    fn usyc_is_disabled_by_default() {
        let caps = RuntimeCapabilities::from_config(&real_cfg());
        assert_eq!(caps.usyc, AdapterCapability::Disabled);
    }

    #[test]
    fn stablefx_is_always_unavailable() {
        let caps = RuntimeCapabilities::from_config(&real_cfg());
        assert!(matches!(caps.stablefx, AdapterCapability::Unavailable(_)));
    }

    #[cfg(not(feature = "real-cctp"))]
    #[test]
    fn cctp_needs_feature_without_real_cctp() {
        let caps = RuntimeCapabilities::from_config(&real_cfg());
        assert_eq!(caps.cctp, AdapterCapability::NeedsFeature);
    }

    #[cfg(not(feature = "real-swap"))]
    #[test]
    fn swap_needs_feature_without_real_swap() {
        let caps = RuntimeCapabilities::from_config(&real_cfg());
        assert_eq!(caps.swap, AdapterCapability::NeedsFeature);
    }
}
