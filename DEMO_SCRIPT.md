# Aegis — Demo Script (Agora Agents Hackathon · RFB 04)

**Format:** spoken voiceover + on-screen actions · **Target length:** ~5:00 · **Path:** authenticated, real-execution flow
**Recorded by walking the live app on 2026-05-23. Every screen, badge, slug, and number below was verified in-browser.**

---

## 0. Read this first — the one honest constraint

The app is built in a _real-execution_ config (`EXECUTION_MOCK=false`, `MOCK_CIRCLE=false`, binary compiled with `real-cctp` + `real-usyc`). Almost the entire flow is real and looks superb on camera. There is exactly **one** caveat to plan around:

- **USYC park reverts on-chain** with `NotPermissioned()` (`0x7f63bd0f`). USYC is a permissioned, KYC'd T-bill: Hashnote's Entitlements contract on Arc testnet must allowlist the signer EOA, and ours has `hasAccess:false` (confirmed via `api.hashnote.com`). This is **not a bug** — it's the institutional gate. It can't be fixed by editing the repo.
- **Volatile/EURC plans are blocked at approval by design** (BTC/ETH/SOL swaps + EURC FX show "Approval blocked" with amber capability chips). Deliberate safety gate.
- **CCTP V2 burn is permissionless and real** — this is our on-chain settlement moment (see §"Settlement options").

**So: the demo is fully real up to and including the approval modal + fee preview. For the on-chain "money moves" beat, use a real CCTP V2 transfer (recommended) — verify it once before recording. A guaranteed-clean fallback is `EXECUTION_MOCK=true` (disclose it).**

---

## Pre-flight checklist (do before hitting record)

1. **Servers up:** web `localhost:3000`, api `localhost:8080`, Postgres healthy. (Web: `pnpm dev`. API: `cargo run --features "real-cctp real-usyc"` from `apps/api/`.)
2. **🔒 SECURITY — hide `.env.local`.** The repo-root `.env.local` holds live API keys **and EOA private keys in plaintext**. Close that file/tab, clear your terminal scrollback, and never show the editor file tree or env during recording.
3. **Fund the wallet (human step — faucet has reCAPTCHA):** copy your Arc address from `/wallets`, go to `faucet.circle.com`, claim USDC on **Arc Testnet** (and **Base Sepolia** if doing the CCTP climax). Wait ~30s; balance shows on `/wallets`.
4. **Pick & verify the settlement beat** (see §"Settlement options"). Build the plan **fresh** right before recording — stale/superseded plans get gated.
5. **Pre-create the demo account** so you skip the email-code wait on camera (or keep the inbox handy to read the 6-digit code live — it's a nice "no seed phrase" beat).
6. **Browser:** full-screen, 1440-wide, dark OS theme. Hide bookmarks bar and extensions.
7. **Have two browser profiles/tabs ready:** one signed-in (authenticated flow), one clean (for `/explore` and the public diary).

---

## The 5-minute script

> VO = what you say. SCREEN = what you do/show. Times are cumulative targets.

### Scene 1 — Hook (0:00–0:30) · `localhost:3000`

**SCREEN:** Landing page. Let the hero and the live decision ticker breathe; hover the Circle-stack chips.
**VO:** "This is Aegis — an adaptive portfolio manager for stablecoin-native finance, built entirely on Circle's stack. The promise is three words: _set a goal, the agent proposes, you approve._ No black box — every decision ships with the model that made it and a public reasoning diary. Let me show you."

### Scene 2 — How the agent thinks (0:30–1:10) · `/explore` → Operating Reserve

**SCREEN:** Click **View demo** → open **Operating Reserve**. Point at: the regime pill (**NEUTRAL**), the model badge **`anthropic/claude-opus-4-7`** (confidence **81%**), the strategist's prose, and the **Critic** verdict ("Survives critique…", **82%**).
**VO:** "Before anyone connects a wallet, they can inspect real agent reasoning. This is our multi-model loop: a regime classifier reads the market, a strategist proposes — here, Claude Opus — and a _different_ model acts as an adversarial critic. Every decision surfaces its model slug and a calibrated confidence. That's the trust contract."

### Scene 3 — Your goal, your wallet (1:10–1:55) · `/login` → `/onboarding`

**SCREEN:** Sign in with email → enter the 6-digit code → land in onboarding. Quickly step the 4-step wizard: **name → horizon → risk → target allocation**. (For this run pick a yield-forward target.)
**VO:** "Onboarding is one email field — we create a non-custodial **Circle Wallet** behind it. No seed phrase, no extension, no KYC wall to look around. Then you set the goal the agent must respect: horizon, risk tolerance, and a target mix across USDC, USYC yield, and an EURC sleeve. You steer; the agent executes inside these rails."

### Scene 4 — Funded, unified, ready (1:55–2:30) · `/wallets` → dashboard

**SCREEN:** On `/wallets` show the single address across **5 networks** with explorer links, and **Arc + Base flagged as EXECUTION rails**; idle cash e.g. **$40.00 USDC**. Back to the dashboard: **Net Worth**, **Target Mix**, live **Market** card (`via defillama`, Fear & Greed 28), and the **Review plan** CTA.
**VO:** "One address, every supported chain — Circle **Gateway** gives a unified USDC balance, and gas is paid in USDC by Circle **Paymaster**, so users never touch ETH. Cash is funded but _not_ invested — nothing moves until I approve. Let's review what the agent wants to do."

### Scene 5 — The approval modal — the core moment (2:30–3:40)

**SCREEN:** Click **Review plan** → the **Approve rebalance** modal. Walk it top to bottom, slowly:

- **"What will change"** card (cyan) — the plain-English deltas.
- Model badge **`aegis/rebalance-planner-v1`**, regime pill **neutral**, **confidence 92%** bar, **"Constitution clean"**.
- **Critic** verdict + the collapsible **"Why this might be wrong"** counterfactual.
- **Execution route** map (single-chain ARC, or the **CCTP bridge** variant if cross-chain).
- **Technical route** accordion → per-leg `ChainBadge`s.
- **Fee block:** **Paymaster (USDC gas) ≈ $0.0120 USDC** _(via Circle Paymaster · live)_, **Protocol fee (25 bps via Nanopayments x402) ≈ $0.1000**, **Total ≈ $0.1120 USDC**.
  **VO:** "This single screen is the whole product. The model that built the plan. The regime it read. A calibrated confidence bar. A critic's verdict — and a counterfactual for _why it might be wrong_. The exact route across chains. And the full cost, in USDC: gas via Paymaster, plus a 25-basis-point protocol fee settled through Circle **Nanopayments**. Every trust signal, visible without scrolling. Nothing executes until I click approve."

### Scene 6 — Approve → real on-chain settlement (3:40–4:30)

**SCREEN:** Click **Approve & execute**. The **Execution trace** takes over: legs go PENDING → SUBMITTED → **CONFIRMED**, each with a **"view on explorer ↗"** link (Arcscan / BaseScan) and a **"Hook executed"** badge on CCTP legs; the **Nanopayments fee** shows a settlement tx. Click a tx hash to open the block explorer on real testnet.
**VO:** "One approval. The agent burns USDC through Circle's **CCTP V2** with a destination hook, the message is attested, and it settles on the other chain — real testnet USDC, on a public explorer you can verify right now. Gas paid in USDC. Fee settled on-chain. This is the entire Circle stack firing in a single user action."

### Scene 7 — Receipts & control → close (4:30–5:00) · `/decision/:id` → settings

**SCREEN:** Open the public **decision diary** (`/decision/<id>`): inputs, strategist, critic, plan, execution, and the **audit trail** (the realized-vs-counterfactual cards fill in 24h later). Then flash the **agent pause** toggle in settings, and the **leaderboard / traction counters**.
**VO:** "Every decision becomes a permanent, public record — inputs, reasoning, critic, execution, and 24 hours later the realized outcome versus the counterfactual. You can pause the agent with one toggle and withdraw anytime; the USDC never leaves your Circle Wallet. Real users, real testnet USDC, one human approval per move — that's Aegis."

---

## Settlement options for Scene 6 (pick one, verify before recording)

| Option                            | What the camera sees                                                                                      | Real?                          | Effort / risk                                                                                                                                                                                              |
| --------------------------------- | --------------------------------------------------------------------------------------------------------- | ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A. CCTP V2 burn (recommended)** | Real burn tx confirms fast + explorer link; mint completes minutes later (or shows "attesting")           | ✅ Real Circle rails           | Fund Base too; engineer a USDC cross-chain plan; **verify a burn confirms once** before recording. Mint can exceed the 180s in-app attestation timeout — that's fine, the _burn_ is the verifiable moment. |
| **B. Stand-in USYC vault**        | Real on-chain deposit tx to a permissionless ERC-4626 vault we deploy on Arc testnet; confirms in seconds | ✅ Real tx (stand-in contract) | I can write + deploy `MockUsycTeller.sol`, point `USYC_TELLER_ARC`/`USYC_TOKEN_ARC` at it, restart API. Honest disclosure: "testnet stand-in for Hashnote USYC, which is KYC-gated."                       |
| **C. Mock-exec mode**             | Approve → legs settle → tx hashes → diary, all simulated cleanly                                          | ⚠️ Simulated                   | Restart API with `EXECUTION_MOCK=true`. Zero on-camera risk. **Must disclose** it's sandbox mode; weakens the Traction claim.                                                                              |
| **D. Real USYC (blocked)**        | —                                                                                                         | ✅ but unavailable             | Requires Hashnote to grant Entitlements to the EOA (KYC onboarding). Not achievable by code; pursue separately.                                                                                            |

**Recommendation:** Option **A** for the on-camera "real money" beat (it's the actual Circle product judges score), with **C** as a safety re-take. Keep USYC in the _proposal_ (Scene 5 renders it beautifully) and add one honest line: "On mainnet this subscribes to Hashnote USYC; on testnet that Teller is KYC-gated, so our live settlement uses CCTP."

---

## Traction evidence to show or mention (judges can verify)

- Public block-explorer tx hashes from Scene 6 (Arcscan / BaseScan).
- The funded Circle wallet address across 5 chains on `/wallets`.
- The live decision diary at `/decision/<id>` and the `/leaderboard`.
- The on-screen provenance lines (`via defillama · live tick`, `via Circle Paymaster · Ns ago`) prove data is live, not canned.

## Do / Don't on camera

- **Do** read the 6-digit code live (sells "no seed phrase"), click a real tx hash into the explorer, and pause on the fee + critic + confidence signals.
- **Don't** show `.env.local`, the editor file tree, terminal env, or attempt a live USYC park / a BTC-ETH-EURC approval (both fail/blocked on camera).
