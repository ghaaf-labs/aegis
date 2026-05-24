use sha2::{Digest, Sha256};

use crate::modules::rebalance::models::ChainKey;

use super::{Attestation, BurnReceipt, HookPayload};

pub(super) fn mock_tx_hash(kind: &str, chain: &str, salt: &str) -> String {
    let mut h = Sha256::new();
    h.update(kind.as_bytes());
    h.update(b":");
    h.update(chain.as_bytes());
    h.update(b":");
    h.update(salt.as_bytes());
    format!("0x{}", hex::encode(h.finalize()))
}

pub(super) fn mock_burn_receipt(
    src: ChainKey,
    dest: ChainKey,
    amount: f64,
    hook: &HookPayload,
) -> BurnReceipt {
    let salt = format!(
        "{}->{}:{}:{}",
        src.as_str(),
        dest.as_str(),
        amount,
        hook.recipient
    );
    let mut h = Sha256::new();
    h.update(b"msg:");
    h.update(salt.as_bytes());
    let message_hash = format!("0x{}", hex::encode(h.finalize()));
    BurnReceipt {
        tx_hash: mock_tx_hash("burn", src.as_str(), &salt),
        message_hash,
    }
}

pub(super) fn mock_attestation(message_hash: &str) -> Attestation {
    Attestation {
        message: format!("0x{}", hex::encode(message_hash.as_bytes())),
        attestation: format!(
            "0x{}",
            hex::encode(format!("att:{message_hash}").as_bytes())
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::rebalance::cross_chain::build_hook_payload;

    #[tokio::test]
    async fn mock_burn_is_deterministic() {
        let http = reqwest::Client::new();
        let cfg = crate::config::test_config();
        let client = super::super::CctpClient::new(&http, &cfg);
        let hook = build_hook_payload("0xabc", "0xdef", 3000, 1_000_000, 1_700_000_000);

        let r1 = client
            .deposit_for_burn(ChainKey::Arc, ChainKey::Base, 100.0, &hook)
            .await
            .unwrap();
        let r2 = client
            .deposit_for_burn(ChainKey::Arc, ChainKey::Base, 100.0, &hook)
            .await
            .unwrap();
        assert_eq!(r1, r2, "mock burn must be deterministic");
        assert!(r1.tx_hash.starts_with("0x"));
        assert!(r1.message_hash.starts_with("0x"));
    }

    #[tokio::test]
    async fn mock_attestation_roundtrips() {
        let http = reqwest::Client::new();
        let cfg = crate::config::test_config();
        let client = super::super::CctpClient::new(&http, &cfg);
        let hook = build_hook_payload("0xabc", "0xdef", 3000, 1, 1);
        let burn = client
            .deposit_for_burn(ChainKey::Arc, ChainKey::Base, 50.0, &hook)
            .await
            .unwrap();
        let att = client
            .wait_for_attestation(ChainKey::Arc.domain_id(), &burn.tx_hash)
            .await
            .unwrap();
        assert!(!att.message.is_empty());
        assert!(!att.attestation.is_empty());
        let mint = client.receive_message(ChainKey::Base, &att).await.unwrap();
        assert!(mint.tx_hash.starts_with("0x"));
    }
}
