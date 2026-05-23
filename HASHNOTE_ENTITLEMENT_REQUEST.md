# Hashnote / Circle — USYC Arc-testnet entitlement request

**Purpose:** get our integration wallet allowlisted (entitled) for USYC on **Arc testnet** so it can subscribe/redeem via the Teller. Today the Teller reverts `NotPermissioned()` because the wallet is not entitled.

**Where to send:** Hashnote support / the Build-on-Circle Discord (`discord.com/invite/buildoncircle`) and/or your Circle hackathon contact for Agora Agents (RFB 04). Attach this file or paste the short version.

---

## The ask (one sentence)

Please grant USYC **Entitlements / token access** on **Arc testnet** to the wallet address below so it can deposit (subscribe) and redeem through the USYC Teller for a Circle hackathon integration.

## Technical details

| Field                      | Value                                                                                                          |
| -------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Project                    | **Aegis** — adaptive stablecoin portfolio manager (Agora Agents Hackathon, RFB 04: Adaptive Portfolio Manager) |
| Network                    | **Arc testnet** (chain id `5042002`)                                                                           |
| Wallet to entitle (EOA)    | `0xea07c870eb552919e23afa0a0f620d87a344461b`                                                                   |
| USYC token (Arc testnet)   | `0xe9185F0c5F296Ed1797AaE4238D26CCaBEadb86C`                                                                   |
| USYC Teller (Arc testnet)  | `0x9fdF14c5B14173D74C08Af27AebFf39240dC105A`                                                                   |
| USYC Oracle (Arc testnet)  | `0x52b56c7642E71dc54714d879127d97cd0B3D4581`                                                                   |
| Entitlements (Arc testnet) | `0xcc205224862c7641930c87679e98999d23c26113`                                                                   |
| USDC (Arc)                 | `0x3600000000000000000000000000000000000000`                                                                   |

## What we observe today

- Calling `Teller.deposit(uint256 assets, address receiver)` (after `USDC.approve`) reverts with custom error **`NotPermissioned()`** — selector **`0x7f63bd0f`** — i.e. `execution reverted, data: "0x7f63bd0f"`.
- Hashnote's entitlement API confirms the wallet is not allowlisted:
  ```
  GET https://api.hashnote.com/v1/entitlements/token_access?address=0xea07c870eb552919e23afa0a0f620d87a344461b&symbol=USYC
  → {"entity":"token_access","data":{"symbol":"USYC","address":"0xea07c870…461b","hasAccess":false}}
  ```

## What we're building (use case)

Aegis lets a user set a portfolio goal; a multi-model AI agent proposes rebalances that the user approves. One sleeve parks idle USDC into **USYC** for treasury yield via the Teller (subscribe), and redeems back to USDC when the agent rebalances. We need testnet entitlement to demonstrate the live USDC↔USYC subscribe/redeem path end-to-end for hackathon judging.

## Questions for Hashnote/Circle

1. Is there a **self-service testnet entitlement** flow, or must you grant it manually? If self-service, what function / API / form do we use?
2. If manual, can you entitle the EOA above for **token_access (hold) + Teller subscribe/redeem** on Arc testnet?
3. Any minimum amounts, lock-ups, or oracle/price-staleness constraints we should expect on testnet subscribe/redeem?

---

### Short version (paste into Discord / chat)

> Building **Aegis** for the Agora Agents Hackathon (RFB 04) on **Arc testnet**. Our integration wallet `0xea07c870eb552919e23afa0a0f620d87a344461b` gets `NotPermissioned()` (`0x7f63bd0f`) when calling the USYC Teller `0x9fdF14c5B14173D74C08Af27AebFf39240dC105A`, and `api.hashnote.com/.../token_access` returns `hasAccess:false`. **Can you grant USYC token-access / Teller entitlement to that address on Arc testnet?** Or point us at a self-service testnet allowlist if one exists. Thanks!

### Once granted — verify with:

```
GET https://api.hashnote.com/v1/entitlements/token_access?address=0xea07c870eb552919e23afa0a0f620d87a344461b&symbol=USYC
# expect: "hasAccess": true
```

Then a USYC park in Aegis (USDC→USYC, single-chain Arc) will settle on-chain instead of reverting.
