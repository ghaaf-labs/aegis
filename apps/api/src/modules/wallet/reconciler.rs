use tokio::time::{Duration, MissedTickBehavior};
use tokio_util::sync::CancellationToken;

use super::{
    provider::{CircleProvider, MockProvider},
    service::WalletService,
};
use crate::router::AppState;

const WALLET_RECONCILE_INTERVAL_SECS: u64 = 60;
const WALLET_RECONCILE_BATCH_SIZE: i64 = 25;

pub fn spawn_provisioning_reconciler(state: AppState, cancel: CancellationToken) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(WALLET_RECONCILE_INTERVAL_SECS));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = ticker.tick() => {
                    match reconcile_once(&state).await {
                        Ok(healed) if healed > 0 => {
                            tracing::info!(healed, "wallet provisioning reconciler healed accounts");
                        }
                        Ok(_) => {}
                        Err(e) => tracing::warn!(error=%e, "wallet provisioning reconciler failed"),
                    }
                }
            }
        }
    });
}

async fn reconcile_once(state: &AppState) -> crate::error::Result<usize> {
    if state.config.circle_mock {
        let provider = MockProvider;
        let service = WalletService::new(&state.db, &provider, &state.config, &state.sse);
        return service
            .reconcile_pending_wallets(WALLET_RECONCILE_BATCH_SIZE)
            .await;
    }

    let provider = CircleProvider::new(&state.http, &state.config);
    let service = WalletService::new(&state.db, &provider, &state.config, &state.sse);
    service
        .reconcile_pending_wallets(WALLET_RECONCILE_BATCH_SIZE)
        .await
}
