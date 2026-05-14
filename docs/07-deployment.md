# Aegis · Deployment

Aegis ships as two services (Rust API + Next.js web) plus Postgres. We deploy
to **our own servers** — no Vercel, no PostHog, no managed-anything. Two
canonical paths:

- **docker-compose** (`infra/docker/docker-compose.prod.yml`) — single VPS,
  Caddy reverse proxy with automatic TLS via Let's Encrypt. Fastest path
  to a live URL.
- **k3s** (`infra/k3s/*.yaml`) — multi-node, rolling deploys, Traefik
  ingress + cert-manager. Use when you need more than one web replica.

## Quickstart (docker-compose)

```bash
# 1. Build images
docker compose -f infra/docker/docker-compose.prod.yml build

# 2. Set the env. ${AEGIS_DOMAIN} must already resolve to this host.
export AEGIS_DOMAIN=aegis.example.com
export POSTGRES_PASSWORD=$(openssl rand -hex 24)
export JWT_SECRET=$(openssl rand -hex 32)
export DIGEST_SECRET=$(openssl rand -hex 32)
export OPENROUTER_API_KEY=sk-or-…

# 3. Bring it up. Caddy will obtain a Let's Encrypt cert on first request.
docker compose -f infra/docker/docker-compose.prod.yml up -d

# 4. Migrate the DB.
docker compose -f infra/docker/docker-compose.prod.yml exec api \
  cargo sqlx migrate run
```

Verify: `curl -fsSL https://${AEGIS_DOMAIN}/api/health`.

## k3s

```bash
# 1. Secrets
kubectl create secret generic aegis-api \
  --from-literal=database-url="postgres://aegis:…@postgres:5432/aegis" \
  --from-literal=jwt-secret="$(openssl rand -hex 32)" \
  --from-literal=digest-secret="$(openssl rand -hex 32)" \
  --from-literal=openrouter-api-key="sk-or-…" \
  --from-literal=circle-api-key="…" \
  --from-literal=chain-private-key-arc="0x…" \
  --from-literal=chain-private-key-base="0x…" \
  --from-literal=resend-api-key="re_…"

kubectl create secret generic aegis-postgres --from-literal=password="…"

kubectl create configmap aegis-config --from-literal=domain=aegis.example.com

# 2. Apply manifests in order.
kubectl apply -f infra/k3s/postgres.yaml
kubectl apply -f infra/k3s/aegis-api.yaml
kubectl apply -f infra/k3s/aegis-web.yaml
kubectl apply -f infra/k3s/ingress.yaml
```

## Flipping `EXECUTION_MOCK=false` (real CCTP testnet)

The cross-chain executor ships with two modes:

| Mode           | `EXECUTION_MOCK` | What `cross_chain::deposit_for_burn` does                                         |
| -------------- | ---------------- | --------------------------------------------------------------------------------- |
| Mock (default) | `true`           | Returns deterministic mock receipts. Tests + CI use this.                         |
| Real           | `false`          | Submits signed `depositForBurnWithCaller` to the source chain's `TokenMessenger`. |

Before flipping:

1. **Deploy `RebalanceExecutor.sol` to Arc + Base Sepolia**:

   ```bash
   forge create RebalanceExecutor \
     --rpc-url $ARC_RPC_URL \
     --private-key $CHAIN_PRIVATE_KEY_ARC \
     --constructor-args $MESSAGE_TRANSMITTER_ARC $UNI_V3_ROUTER_ARC $OWNER_ADDR

   forge create RebalanceExecutor \
     --rpc-url $BASE_RPC_URL \
     --private-key $CHAIN_PRIVATE_KEY_BASE \
     --constructor-args $MESSAGE_TRANSMITTER_BASE $UNI_V3_ROUTER_BASE $OWNER_ADDR
   ```

2. **Fund operator wallets** with native gas (Arc USDC; Base ETH from
   the Base Sepolia faucet) and 100+ USDC for swaps. The `CHAIN_PRIVATE_KEY_*`
   accounts pay all transaction fees.

3. **Update `packages/shared/src/constants.ts`**:

   ```ts
   export const CHAIN_ADDRESSES = {
     arc: {
       executor: "0x…",  // the deployed RebalanceExecutor
       tokenMessenger: "…",
       messageTransmitter: "…",
       usdc: "…",
     },
     base: { executor: "0x…", … },
   };
   ```

4. **Wire the real `alloy` calls.** `cross_chain.rs::deposit_for_burn`
   currently returns an `AppError::Internal` when `EXECUTION_MOCK=false`.
   Replace with the production path:

   ```rust
   use alloy::{
       primitives::{Address, Bytes, U256},
       providers::{Provider, ProviderBuilder},
       signers::local::PrivateKeySigner,
       sol,
   };

   sol! {
       #[sol(rpc)]
       contract ICCTPV2TokenMessenger {
           function depositForBurnWithCaller(
               uint256 amount,
               uint32 destinationDomain,
               bytes32 mintRecipient,
               address burnToken,
               bytes32 destinationCaller
           ) external returns (uint64 nonce);
       }
   }

   let signer = PrivateKeySigner::from_str(&self.config.chain_private_key_arc)?;
   let provider = ProviderBuilder::new()
       .with_recommended_fillers()
       .signer(signer.into())
       .on_http(self.config.arc_rpc_url.parse()?);
   let contract = ICCTPV2TokenMessenger::new(token_messenger_addr, &provider);
   let receipt = contract
       .depositForBurnWithCaller(
           U256::from(amount_units),
           dest.domain_id(),
           bytes32(executor_on_dest),
           usdc_addr,
           bytes32(executor_on_dest),
       )
       .send().await?
       .get_receipt().await?;
   let message_hash = decode_message_sent(&receipt)
       .ok_or_else(|| anyhow::anyhow!("MessageSent log not found"))?;
   ```

   The `alloy` crate is intentionally **not** in `Cargo.toml` by default —
   it adds ~50 crates to the build and isn't reachable from any code path
   under `EXECUTION_MOCK=true`. Add it when you wire the real path:

   ```toml
   alloy = { version = "0.5", features = ["full"] }
   ```

5. **`Config::validate()` will refuse to boot** if `EXECUTION_MOCK=false`
   but the private keys are empty. This is intentional — the binary won't
   silently fall back to mock receipts in production.

## Smoke test (testnet end-to-end)

```bash
# 1. Local
docker compose -f infra/docker/docker-compose.prod.yml up -d
curl https://${AEGIS_DOMAIN}/api/health

# 2. Create a passkey wallet, fund 100 USDC.
#    (run from the browser at /signup)

# 3. Open a portfolio, force a regime flip.
curl -X POST -H "Authorization: Bearer $TOKEN" \
  https://${AEGIS_DOMAIN}/api/agent/analyze \
  -d '{"portfolioId":"…", "triggeredBy":"regime_flip"}'

# 4. Approve the plan via the UI. Watch the SSE stream.
#    Verify on Arc Sepolia explorer: MessageSent.
#    Verify on Base Sepolia explorer: MessageReceived + Swap + USDC transfer.
```

## Rolling back

```bash
# docker-compose
docker compose -f infra/docker/docker-compose.prod.yml pull web:previous-tag
docker compose -f infra/docker/docker-compose.prod.yml up -d --no-deps web

# k3s
kubectl rollout undo deployment/aegis-api
kubectl rollout undo deployment/aegis-web
```

## What's intentionally **not** in this deploy

- **Vercel / cloud SaaS** — by design. The whole stack runs on our hardware.
- **PostHog / Mixpanel / Amplitude** — analytics live in our own
  `analytics_events` Postgres table, queried directly. See
  `apps/api/src/modules/analytics/`.
- **Background AI training** — every decision is fresh-prompted via
  OpenRouter; we don't fine-tune.
- **External secrets manager** — Kubernetes secrets + docker-compose env
  files are enough for the hackathon. For production, swap in SOPS or
  Vault before the first real-money launch.
