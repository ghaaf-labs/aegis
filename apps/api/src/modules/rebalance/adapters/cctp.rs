//! CCTP V2 bridge adapter — USDC burn on the source chain, mint on the
//! destination. Wraps [`CctpClient`] (which holds the real alloy path behind
//! `real-cctp`) and reports its capability from `Config`.

use uuid::Uuid;

use crate::config::Config;
use crate::db::Db;
use crate::error::Result;

use super::super::cross_chain::{CctpClient, HookPayload};
use super::super::models::ChainKey;
use super::super::registry::capabilities::AdapterCapability;
use super::super::registry::ticket::ExecutionTicket;
use super::super::registry::tokens::{self, USDC};
use super::RealReceipt;

/// Capability of the CCTP V2 USDC bridge (Arc ↔ Base). Requires *every*
/// address the real burn/mint path parses — USDC, TokenMessengerV2,
/// MessageTransmitterV2, and the destination RebalanceExecutor on both chains —
/// so a leg fails closed at approval rather than late at submit time.
pub fn capability(cfg: &Config) -> AdapterCapability {
    let usdc = tokens::token(USDC).expect("USDC in registry");
    let have_addrs = usdc.address_for(cfg, ChainKey::Arc).is_some()
        && usdc.address_for(cfg, ChainKey::Base).is_some()
        && tokens::is_real_addr(&cfg.cctp_token_messenger_arc)
        && tokens::is_real_addr(&cfg.cctp_token_messenger_base)
        && tokens::is_real_addr(&cfg.cctp_message_transmitter_arc)
        && tokens::is_real_addr(&cfg.cctp_message_transmitter_base)
        && tokens::is_real_addr(&cfg.rebalance_executor_arc)
        && tokens::is_real_addr(&cfg.rebalance_executor_base);
    if !cfg!(feature = "real-cctp") {
        AdapterCapability::NeedsFeature
    } else if !have_addrs {
        AdapterCapability::NeedsAddress
    } else if cfg.chain_private_key_arc.trim().is_empty()
        || cfg.chain_private_key_base.trim().is_empty()
    {
        AdapterCapability::NeedsSigner
    } else {
        AdapterCapability::Live
    }
}

/// Backend EOA address on `chain`, derived from its signing key. In the
/// custodial execution path (`circle_wallet_exec = false`) the backend signer
/// holds the USDC in motion and performs the destination swap, so a cross-chain
/// mint must be delivered to *it* — not to a per-user Circle wallet (which a
/// synthetic/EOA user may not have). `None` when real-cctp is off or the key is
/// unset/invalid, in which case the burn fails closed before the recipient is
/// used.
#[cfg(feature = "real-cctp")]
pub fn eoa_address_for(cfg: &Config, chain: ChainKey) -> Option<String> {
    use alloy::signers::local::PrivateKeySigner;
    let bytes = hex::decode(cfg.chain_private_key_for(chain).trim_start_matches("0x")).ok()?;
    let signer = PrivateKeySigner::from_slice(&bytes).ok()?;
    Some(signer.address().to_string())
}

#[cfg(not(feature = "real-cctp"))]
pub fn eoa_address_for(_cfg: &Config, _chain: ChainKey) -> Option<String> {
    None
}

/// Burn USDC on the source chain, forwarding `hook` so the destination
/// RebalanceExecutor can act on mint. Real path only — gated by the ticket.
pub async fn burn(
    cfg: &Config,
    http: &reqwest::Client,
    db: &Db,
    user_id: Uuid,
    ticket: &ExecutionTicket,
    hook: &HookPayload,
) -> Result<RealReceipt> {
    let client = CctpClient::new(http, cfg).with_user(db, user_id);
    let r = client
        .deposit_for_burn(
            ticket.src_chain(),
            ticket.dest_chain(),
            ticket.amount_usdc(),
            hook,
        )
        .await?;
    Ok(RealReceipt {
        tx_hash: r.tx_hash,
        cctp_message_hash: Some(r.message_hash),
    })
}

/// Wait for the attestation produced by `burn_tx_hash` and mint on the
/// destination chain.
pub async fn mint(
    cfg: &Config,
    http: &reqwest::Client,
    db: &Db,
    user_id: Uuid,
    ticket: &ExecutionTicket,
    burn_tx_hash: &str,
) -> Result<RealReceipt> {
    let client = CctpClient::new(http, cfg).with_user(db, user_id);
    let att = client
        .wait_for_attestation(ticket.src_chain().domain_id(), burn_tx_hash)
        .await?;
    let r = client.receive_message(ticket.dest_chain(), &att).await?;
    Ok(RealReceipt {
        tx_hash: r.tx_hash,
        cctp_message_hash: None,
    })
}
