//! Wallet service — orchestrates email-code auth, Circle wallet persistence,
//! and opaque session minting.

pub mod auth_code;
pub mod consent;
pub mod provisioning;

pub use auth_code::enforce_auth_ip_rate_limit;

use super::provider::WalletProvider;
use crate::config::Config;
use crate::db::Db;
use crate::modules::sse::SseSender;

pub const CURRENT_TOS_VERSION: &str = "2026-05";
pub const CURRENT_PRIVACY_VERSION: &str = "2026-05";

pub struct WalletService<'a> {
    pub db: &'a Db,
    pub provider: &'a dyn WalletProvider,
    pub config: &'a Config,
    pub sse: &'a SseSender,
}

pub struct WalletAuthCodeIssue {
    pub response: super::models::WalletAuthCodeResponse,
    pub code: String,
}

pub(super) struct AuthCodeCheck {
    pub(super) email: String,
    pub(super) code_hash: String,
    pub(super) referrer_handle: Option<String>,
}

pub struct VerifiedAuthCode {
    pub email: String,
    pub referrer_handle: Option<String>,
}

impl<'a> WalletService<'a> {
    pub fn new(
        db: &'a Db,
        provider: &'a dyn WalletProvider,
        config: &'a Config,
        sse: &'a SseSender,
    ) -> Self {
        Self {
            db,
            provider,
            config,
            sse,
        }
    }
}
