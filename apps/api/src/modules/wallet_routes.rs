use sqlx::Row;
use uuid::Uuid;

use crate::db::Db;

pub const ARC_TESTNET: &str = "ARC-TESTNET";
pub const BASE_SEPOLIA: &str = "BASE-SEPOLIA";
pub const ETH_SEPOLIA: &str = "ETH-SEPOLIA";
pub const ARB_SEPOLIA: &str = "ARB-SEPOLIA";
pub const AVAX_FUJI: &str = "AVAX-FUJI";

pub const SUPPORTED_WALLET_BLOCKCHAINS: [&str; 5] = [
    ARC_TESTNET,
    BASE_SEPOLIA,
    ETH_SEPOLIA,
    ARB_SEPOLIA,
    AVAX_FUJI,
];

pub const EXECUTION_BLOCKCHAINS: [&str; 2] = [ARC_TESTNET, BASE_SEPOLIA];

pub async fn address_for_user(
    db: &Db,
    user_id: Uuid,
    blockchain: &str,
    wallet_set_id: &str,
) -> crate::error::Result<Option<String>> {
    sqlx::query_scalar(
        "SELECT address
         FROM user_wallet_networks
         WHERE user_id = $1
           AND blockchain = $2
           AND account_type = 'SCA'
           AND state = 'LIVE'
           AND ($3 = '' OR wallet_set_id = $3)
         LIMIT 1",
    )
    .bind(user_id)
    .bind(blockchain)
    .bind(wallet_set_id.trim())
    .fetch_optional(db)
    .await
    .map_err(Into::into)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WalletRouteAddress {
    pub blockchain: String,
    pub address: String,
}

pub async fn addresses_for_user(
    db: &Db,
    user_id: Uuid,
    wallet_set_id: &str,
) -> crate::error::Result<Vec<WalletRouteAddress>> {
    sqlx::query_as::<_, WalletRouteAddress>(
        "SELECT blockchain, address
         FROM user_wallet_networks
         WHERE user_id = $1
           AND blockchain IN (
             'ARC-TESTNET',
             'BASE-SEPOLIA',
             'ETH-SEPOLIA',
             'ARB-SEPOLIA',
             'AVAX-FUJI'
           )
           AND account_type = 'SCA'
           AND state = 'LIVE'
           AND ($2 = '' OR wallet_set_id = $2)
         ORDER BY CASE blockchain
           WHEN 'ARC-TESTNET' THEN 1
           WHEN 'BASE-SEPOLIA' THEN 2
           WHEN 'ETH-SEPOLIA' THEN 3
           WHEN 'ARB-SEPOLIA' THEN 4
           WHEN 'AVAX-FUJI' THEN 5
           ELSE 99
         END",
    )
    .bind(user_id)
    .bind(wallet_set_id.trim())
    .fetch_all(db)
    .await
    .map_err(Into::into)
}

pub async fn address_for_portfolio(
    db: &Db,
    portfolio_id: Uuid,
    blockchain: &str,
    wallet_set_id: &str,
) -> crate::error::Result<Option<String>> {
    sqlx::query_scalar(
        "SELECT n.address
         FROM portfolios p
         JOIN user_wallet_networks n ON n.user_id = p.user_id
         WHERE p.id = $1
           AND n.blockchain = $2
           AND n.account_type = 'SCA'
           AND n.state = 'LIVE'
           AND ($3 = '' OR n.wallet_set_id = $3)
         LIMIT 1",
    )
    .bind(portfolio_id)
    .bind(blockchain)
    .bind(wallet_set_id.trim())
    .fetch_optional(db)
    .await
    .map_err(Into::into)
}

pub async fn arc_address_for_user(
    db: &Db,
    user_id: Uuid,
    wallet_set_id: &str,
) -> crate::error::Result<Option<String>> {
    address_for_user(db, user_id, ARC_TESTNET, wallet_set_id).await
}

pub async fn base_address_for_user(
    db: &Db,
    user_id: Uuid,
    wallet_set_id: &str,
) -> crate::error::Result<Option<String>> {
    address_for_user(db, user_id, BASE_SEPOLIA, wallet_set_id).await
}

pub async fn arc_address_for_portfolio(
    db: &Db,
    portfolio_id: Uuid,
    wallet_set_id: &str,
) -> crate::error::Result<Option<String>> {
    address_for_portfolio(db, portfolio_id, ARC_TESTNET, wallet_set_id).await
}

pub async fn user_has_arc_and_base(
    db: &Db,
    user_id: Uuid,
    wallet_set_id: &str,
) -> crate::error::Result<bool> {
    let rows = sqlx::query(
        "SELECT blockchain, circle_wallet_id, address
         FROM user_wallet_networks
         WHERE user_id = $1
           AND blockchain IN ('ARC-TESTNET', 'BASE-SEPOLIA')
           AND account_type = 'SCA'
           AND state = 'LIVE'
           AND ($2 = '' OR wallet_set_id = $2)",
    )
    .bind(user_id)
    .bind(wallet_set_id.trim())
    .fetch_all(db)
    .await?;

    let mut has_arc = false;
    let mut has_base = false;
    for row in rows {
        let blockchain: String = row.try_get("blockchain")?;
        let wallet_id: String = row.try_get("circle_wallet_id")?;
        let address: String = row.try_get("address")?;
        let ready = !wallet_id.trim().is_empty()
            && !wallet_id.starts_with("mock_wallet_")
            && is_real_evm_address(&address);
        if ready && blockchain == ARC_TESTNET {
            has_arc = true;
        }
        if ready && blockchain == BASE_SEPOLIA {
            has_base = true;
        }
    }

    Ok(has_arc && has_base)
}

pub async fn user_id_for_address(db: &Db, address: &str) -> crate::error::Result<Option<Uuid>> {
    sqlx::query_scalar(
        "SELECT user_id
         FROM user_wallet_networks
         WHERE LOWER(address) = LOWER($1)
         LIMIT 1",
    )
    .bind(address.trim())
    .fetch_optional(db)
    .await
    .map_err(Into::into)
}

fn is_real_evm_address(address: &str) -> bool {
    let Some(hex) = address.trim().strip_prefix("0x") else {
        return false;
    };
    hex.len() == 40 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::is_real_evm_address;

    #[test]
    fn real_evm_address_rejects_placeholders() {
        assert!(is_real_evm_address(
            "0x1111111111111111111111111111111111111111"
        ));
        assert!(!is_real_evm_address("0xARCabc"));
        assert!(!is_real_evm_address("mock_wallet_1"));
        assert!(!is_real_evm_address(""));
    }
}
