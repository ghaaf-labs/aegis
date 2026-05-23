//! All-wallets on-chain transaction ledger.
//!
//! `GET /wallets/transactions` returns the user's real Circle W3S transactions
//! across every provisioned chain wallet — funding deposits, CCTP burns/mints,
//! swaps, and ERC-20 approvals — not just rebalance plans. Each row is
//! normalized to a chain-agnostic shape with an explorer link so the
//! Transactions page is a true multi-wallet ledger.
//!
//! Real-by-default: in mock mode there are no on-chain transactions, so the
//! ledger is empty and the page falls back to its rebalance-plan history.

use axum::{extract::State, Extension, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::Config;
use crate::error::{AppError, Result};
use crate::middleware::auth::Claims;
use crate::modules::rebalance::models::ChainKey;
use crate::modules::wallet_routes::{self, SUPPORTED_WALLET_BLOCKCHAINS};
use crate::router::AppState;

/// One normalized ledger row surfaced to the Transactions page.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerEntry {
    pub id: String,
    /// ISO-8601 creation timestamp as Circle reports it (newest first overall).
    pub date: Option<String>,
    /// Explorer chain key: `arc` | `base` | `eth-sepolia` | `arb-sepolia` | `avax-fuji`.
    pub chain: String,
    /// `deposit` | `bridge` | `swap` | `approve` | `outbound` | `contract`.
    pub kind: String,
    pub token: Option<String>,
    pub amount: Option<String>,
    /// Lower-cased Circle state (`complete`, `confirmed`, `failed`, …).
    pub status: String,
    pub tx_hash: Option<String>,
    pub explorer_url: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<LedgerEntry>>> {
    // Mock mode has no real on-chain activity; the page shows plan history only.
    if state.config.circle_mock {
        return Ok(Json(Vec::new()));
    }

    // Resolve the user's live Circle wallet id per supported chain.
    let mut wallets: Vec<(String, String)> = Vec::new(); // (blockchain, wallet_id)
    for blockchain in SUPPORTED_WALLET_BLOCKCHAINS {
        if let Some(wallet_id) = wallet_routes::wallet_id_for_user(
            &state.db,
            claims.sub,
            blockchain,
            &state.config.circle_wallet_set_id,
        )
        .await?
        {
            wallets.push((blockchain.to_string(), wallet_id));
        }
    }
    if wallets.is_empty() {
        return Ok(Json(Vec::new()));
    }

    let wallet_ids: Vec<String> = wallets.iter().map(|(_, id)| id.clone()).collect();
    let raw = fetch_transactions(&state.http, &state.config, &wallet_ids).await?;

    // Map wallet_id → blockchain so each tx resolves to its chain even though
    // Circle keys transactions by walletId.
    let mut entries: Vec<LedgerEntry> = raw
        .into_iter()
        .map(|tx| normalize(&state.config, &wallets, tx))
        .collect();

    // Newest first across all wallets.
    entries.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(Json(entries))
}

/// Circle W3S `GET /v1/w3s/transactions` filtered to the user's wallets. A
/// permissive struct: fields Circle omits for a given operation decode as
/// `None`/empty rather than failing the whole ledger.
async fn fetch_transactions(
    http: &reqwest::Client,
    config: &Config,
    wallet_ids: &[String],
) -> Result<Vec<CircleTx>> {
    #[derive(Deserialize)]
    struct Envelope {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(default)]
        transactions: Vec<CircleTx>,
    }

    let url = format!("{}/v1/w3s/transactions", config.circle_base_url);
    let resp = http
        .get(&url)
        .query(&[
            ("walletIds", wallet_ids.join(",")),
            ("pageSize", "50".to_string()),
        ])
        .header("Authorization", format!("Bearer {}", config.circle_api_key))
        .header("X-Request-Id", Uuid::new_v4().to_string())
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("circle transactions net: {e}")))?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    }
    let envelope: Envelope = resp
        .error_for_status()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("circle transactions: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("circle transactions decode: {e}")))?;
    Ok(envelope.data.transactions)
}

#[derive(Debug, Deserialize)]
struct CircleTx {
    #[serde(default)]
    id: String,
    #[serde(rename = "walletId", default)]
    wallet_id: String,
    #[serde(default)]
    blockchain: Option<String>,
    #[serde(rename = "txHash", default)]
    tx_hash: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(rename = "transactionType", default)]
    transaction_type: Option<String>,
    #[serde(default)]
    operation: Option<String>,
    #[serde(rename = "createDate", default)]
    create_date: Option<String>,
    #[serde(default)]
    amounts: Vec<String>,
    #[serde(default)]
    amount: Option<String>,
    #[serde(rename = "contractAddress", default)]
    contract_address: Option<String>,
    #[serde(default)]
    token: Option<TokenRef>,
}

#[derive(Debug, Deserialize)]
struct TokenRef {
    #[serde(default)]
    symbol: Option<String>,
}

fn normalize(cfg: &Config, wallets: &[(String, String)], tx: CircleTx) -> LedgerEntry {
    // Prefer the tx's own blockchain, else the chain of the wallet it belongs to.
    let blockchain = tx.blockchain.clone().or_else(|| {
        wallets
            .iter()
            .find(|(_, id)| *id == tx.wallet_id)
            .map(|(bc, _)| bc.clone())
    });
    let chain_key = blockchain.as_deref().and_then(blockchain_to_chain);
    let chain = blockchain
        .as_deref()
        .map(explorer_chain_key)
        .unwrap_or("base")
        .to_string();

    let kind = classify(
        tx.operation.as_deref(),
        tx.transaction_type.as_deref(),
        tx.contract_address.as_deref(),
        cfg,
        chain_key,
    )
    .to_string();

    let token = tx
        .token
        .and_then(|t| t.symbol)
        .or_else(|| token_from_contract(cfg, chain_key, tx.contract_address.as_deref()));

    let amount = tx
        .amounts
        .into_iter()
        .find(|a| !a.trim().is_empty())
        .or(tx.amount);

    let explorer_url = tx.tx_hash.as_deref().map(|h| explorer_tx_url(&chain, h));

    LedgerEntry {
        id: tx.id,
        date: tx.create_date,
        chain,
        kind,
        token,
        amount,
        status: tx.state.unwrap_or_default().to_ascii_lowercase(),
        tx_hash: tx.tx_hash,
        explorer_url,
    }
}

/// Coarse, honest classification from the fields Circle reliably returns.
/// A contract execution is resolved against the configured per-chain addresses
/// (bridge messenger → bridge, swap router → swap, a token contract → approve).
fn classify(
    operation: Option<&str>,
    tx_type: Option<&str>,
    contract: Option<&str>,
    cfg: &Config,
    chain: Option<ChainKey>,
) -> &'static str {
    let inbound = tx_type.is_some_and(|t| t.eq_ignore_ascii_case("INBOUND"));
    match operation {
        Some(op) if op.eq_ignore_ascii_case("CONTRACT_EXECUTION") => {
            classify_contract(contract, cfg, chain)
        }
        Some(op) if op.eq_ignore_ascii_case("TRANSFER") => {
            if inbound {
                "deposit"
            } else {
                "outbound"
            }
        }
        _ if inbound => "deposit",
        _ => "contract",
    }
}

fn classify_contract(
    contract: Option<&str>,
    cfg: &Config,
    chain: Option<ChainKey>,
) -> &'static str {
    let (Some(addr), Some(chain)) = (contract, chain) else {
        return "contract";
    };
    let eq = |configured: &str| !configured.is_empty() && configured.eq_ignore_ascii_case(addr);
    if eq(cfg.cctp_token_messenger_for(chain)) || eq(cfg.cctp_message_transmitter_for(chain)) {
        return "bridge";
    }
    if eq(cfg.swap_router_for(chain)) {
        return "swap";
    }
    // A contract execution that targets a token contract is an ERC-20 approve.
    if token_from_contract(cfg, Some(chain), Some(addr)).is_some() {
        return "approve";
    }
    "contract"
}

/// Resolve a configured token contract address (on `chain`) back to its symbol.
fn token_from_contract(
    cfg: &Config,
    chain: Option<ChainKey>,
    addr: Option<&str>,
) -> Option<String> {
    use crate::modules::rebalance::registry::tokens::TOKEN_REGISTRY;
    let (chain, addr) = (chain?, addr?);
    TOKEN_REGISTRY
        .iter()
        .find(|spec| {
            spec.address_for(cfg, chain)
                .is_some_and(|a| a.eq_ignore_ascii_case(addr))
        })
        .map(|spec| spec.symbol.to_string())
}

/// Circle blockchain slug → the explorer chain key the FE/`lib/explorers.ts` use.
fn explorer_chain_key(blockchain: &str) -> &'static str {
    match blockchain {
        "ARC-TESTNET" | "ARC" => "arc",
        "BASE-SEPOLIA" | "BASE" => "base",
        "ETH-SEPOLIA" => "eth-sepolia",
        "ARB-SEPOLIA" => "arb-sepolia",
        "AVAX-FUJI" => "avax-fuji",
        "OP-SEPOLIA" => "op-sepolia",
        _ => "base",
    }
}

fn blockchain_to_chain(blockchain: &str) -> Option<ChainKey> {
    match blockchain {
        "ARC-TESTNET" | "ARC" => Some(ChainKey::Arc),
        "BASE-SEPOLIA" | "BASE" => Some(ChainKey::Base),
        "ETH-SEPOLIA" => Some(ChainKey::EthSepolia),
        "ARB-SEPOLIA" => Some(ChainKey::ArbSepolia),
        "AVAX-FUJI" => Some(ChainKey::AvaxFuji),
        "OP-SEPOLIA" => Some(ChainKey::OpSepolia),
        _ => None,
    }
}

fn explorer_tx_url(chain: &str, tx_hash: &str) -> String {
    let base = match chain {
        "arc" => "https://testnet.arcscan.app",
        "base" => "https://sepolia.basescan.org",
        "eth-sepolia" => "https://sepolia.etherscan.io",
        "arb-sepolia" => "https://sepolia.arbiscan.io",
        "avax-fuji" => "https://testnet.snowtrace.io",
        "op-sepolia" => "https://sepolia-optimism.etherscan.io",
        _ => "https://sepolia.basescan.org",
    };
    format!("{base}/tx/{tx_hash}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real_cfg() -> Config {
        let mut cfg = crate::config::test_config();
        cfg.cctp_token_messenger_base = "0xBdc0000000000000000000000000000000000001".into();
        cfg.uniswap_v3_router_base = "0x2626664c2603336E57B271c5C0b26F421741e481".into();
        cfg.usdc_base = "0x036CbD53842c5426634e7929541eC2318f3dCF7e".into();
        cfg
    }

    #[test]
    fn classifies_transfers_by_direction() {
        let cfg = real_cfg();
        assert_eq!(
            classify(
                Some("TRANSFER"),
                Some("INBOUND"),
                None,
                &cfg,
                Some(ChainKey::Base)
            ),
            "deposit"
        );
        assert_eq!(
            classify(
                Some("TRANSFER"),
                Some("OUTBOUND"),
                None,
                &cfg,
                Some(ChainKey::Base)
            ),
            "outbound"
        );
    }

    #[test]
    fn classifies_contract_executions_by_target_address() {
        let cfg = real_cfg();
        // Bridge messenger → bridge.
        assert_eq!(
            classify(
                Some("CONTRACT_EXECUTION"),
                Some("OUTBOUND"),
                Some(&cfg.cctp_token_messenger_base),
                &cfg,
                Some(ChainKey::Base),
            ),
            "bridge"
        );
        // Swap router → swap.
        assert_eq!(
            classify(
                Some("CONTRACT_EXECUTION"),
                Some("OUTBOUND"),
                Some(&cfg.uniswap_v3_router_base),
                &cfg,
                Some(ChainKey::Base),
            ),
            "swap"
        );
        // Token contract → approve.
        assert_eq!(
            classify(
                Some("CONTRACT_EXECUTION"),
                Some("OUTBOUND"),
                Some(&cfg.usdc_base),
                &cfg,
                Some(ChainKey::Base),
            ),
            "approve"
        );
        // Unknown contract → generic.
        assert_eq!(
            classify(
                Some("CONTRACT_EXECUTION"),
                Some("OUTBOUND"),
                Some("0x9999999999999999999999999999999999999999"),
                &cfg,
                Some(ChainKey::Base),
            ),
            "contract"
        );
    }

    #[test]
    fn explorer_chain_key_maps_every_supported_chain() {
        assert_eq!(explorer_chain_key("ARC-TESTNET"), "arc");
        assert_eq!(explorer_chain_key("BASE-SEPOLIA"), "base");
        assert_eq!(explorer_chain_key("ETH-SEPOLIA"), "eth-sepolia");
        assert_eq!(explorer_chain_key("ARB-SEPOLIA"), "arb-sepolia");
        assert_eq!(explorer_chain_key("AVAX-FUJI"), "avax-fuji");
    }

    #[test]
    fn token_from_contract_resolves_configured_addresses() {
        let cfg = real_cfg();
        assert_eq!(
            token_from_contract(&cfg, Some(ChainKey::Base), Some(&cfg.usdc_base)),
            Some("USDC".to_string())
        );
        assert_eq!(
            token_from_contract(
                &cfg,
                Some(ChainKey::Base),
                Some("0x9999999999999999999999999999999999999999")
            ),
            None
        );
    }
}
