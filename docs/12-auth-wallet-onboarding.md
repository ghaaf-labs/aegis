# 12 — Auth & Wallet Onboarding — Detailed Specification

> **Type:** detailed product + engineering spec (the "what" and "how it should
> behave"). **Not** an implementation. **Status:** proposal pending one decision
> (§1.3). **Target experience:** _Enter email → verify code → app opens_ — never
> a wallet-SDK setup wizard.

## Contents

1. Summary, scope & the key decision
2. Why the current flow fails (evidence)
3. Requirements (functional + non-functional)
4. Identity & custody model
5. State machine (server, session, client)
6. End-to-end sequences
7. API contract (detailed)
8. Wallet provisioning spec
9. Data model & migrations
10. Security spec & threat model
11. GDPR & legal spec
12. Frontend spec & copy deck
13. Master error catalog
14. Observability & analytics
15. Migration / rollout plan
16. Test plan
17. Acceptance criteria
18. Open questions
19. Glossary

---

## 1. Summary, scope & the key decision

### 1.1 Summary

Replace the current 5-step wallet ceremony (`email → verify → PIN challenge →
poll → done`) and its separate signup/login paths with **one** enumeration-safe
"Continue with email" flow whose only routine screens are **Continue → Enter
code → App**. The wallet is provisioned **server-side with no user interaction**
using **Circle developer-controlled wallets (SCA)**. Account identity is the
verified email; custody model is an attribute of the account, so a future
non-custodial signer attaches to the same identity.

### 1.2 In scope / non-goals

**In scope:** unified auth, server-side wallet provisioning, session lifecycle,
resumable/idempotent signup, GDPR consent + export + erasure, security hardening,
copy, migration, tests.
**Non-goals (this spec):** social/passkey _login_, multi-device session
management UI, self-custody export (all Phase 2); KYC/AML _implementation_ (legal
workstream, flagged in §11.6); the rest of the app beyond the auth/account
surface.

### 1.3 The one decision (blocks build)

**Phase 1 is custodial** (Aegis, via Circle, holds signing authority through an
entity secret). This fits an agent that signs on the user's behalf and matches
the "approve a plan, not each tx" UX. It is also the larger legal commitment
(§11.6). **If custodial is acceptable → this spec stands. If not → we must
front-load the Phase-2 non-custodial signer (§4.3) before launch.**

| Phase         | Model                                                | User action                            | Custody                       | Status      |
| ------------- | ---------------------------------------------------- | -------------------------------------- | ----------------------------- | ----------- |
| **1 (now)**   | Circle developer-controlled, SCA, server-provisioned | email → code → app                     | Aegis/Circle (entity secret)  | this spec   |
| **2 (later)** | Circle Modular passkey wallet, same identity         | opt-in "Take self-custody" in Settings | user (passkey + EOA recovery) | §4.3 sketch |

> **Must-verify before build:** Circle developer-controlled wallets support
> **ARC-TESTNET + BASE-SEPOLIA + ETH-SEPOLIA + ARB-SEPOLIA + AVAX-FUJI** as
> **SCA**. Rebalance execution remains gated to Arc/Base until the matching
> Paymaster/Gas-Station, CCTP V2 Hook executor, RPC, and adapter config is
> deployed for the other routes. SCA is mandatory for Paymaster + CCTP V2 Hooks; the provider
> provisions `accountType: SCA` ([`wallet/provider.rs`](../apps/api/src/modules/wallet/provider.rs)).

---

## 2. Why the prior flow failed (evidence + status)

> **Status (reconciled with code):** the backend cutover has landed — migrations
> `0025`–`0031`, the unified `/auth/email/{start,resend,verify}` + `/auth/session`
>
> - `/account/{export,delete}` routes, and a **developer-controlled** `provider.rs`.
>   The old user-controlled provider and `/auth/wallet/{create,login,readiness,status}`
>   are gone. The web app already shows the unified "Continue with email" entry; the
>   remaining work is the §12.3 copy/jargon cleanup. This section is kept as motivation.

Root causes (pre-cutover): (a) wallet provisioning was an **interactive, async,
failure-prone browser ceremony** (wallets were **user-controlled**); (b) the UI
**exposed internal state** (session-vs-wallet, Circle, Arc/Base, PIN, readiness,
new-vs-returning) as user copy.

Observed live (isolated mock instance):

| Probe                                                    | Result                                                                         | Problem                                                   |
| -------------------------------------------------------- | ------------------------------------------------------------------------------ | --------------------------------------------------------- |
| `POST /auth/wallet/code {intent:"login"}`, unknown email | **HTTP 401**                                                                   | combined entry impossible; enumeration oracle             |
| `POST /auth/wallet/create` ×2 same email                 | **HTTP 422**                                                                   | duplicate signup = unprocessable error, not "sign you in" |
| mock signup response                                     | `wallet:null` + `bundle` while `/status` already had the wallet                | UI still enters polling                                   |
| verify screen copy                                       | _"This email is new, so this code creates your wallet"_ + _"Mock dev code: …"_ | leaks new-vs-returning + dev shortcut + "wallet" jargon   |
| cookie                                                   | `aegis_jwt=<raw JWT>; HttpOnly; SameSite=Lax`                                  | no `__Host-`, no CSRF token, JWT is a portable bearer     |
| post-verify                                              | lands on `/onboarding` with `SESSION VERIFIED` badge                           | internal-state leak                                       |

Code-confirmed stuck states: "Finish wallet setup first" loop, "wallet
provisioning timed out", "Challenge cancelled", "Logout did not finish… do not
request a fresh code", "Email sender is not ready", "Session not accepted".

---

## 3. Requirements

### 3.1 Functional

- **F1** One email field signs in _or_ creates an account based on server state.
- **F2** Verification by a 6-digit emailed code.
- **F3** Wallet provisioned automatically, server-side, no user step.
- **F4** Half-finished accounts resume cleanly on next visit (no dead-end).
- **F5** Returning users restore their session and land in the app.
- **F6** Logout fully ends the session; re-login works immediately.
- **F7** Capture versioned ToS/Privacy consent + separate marketing opt-in.
- **F8** User can export their data and delete their account (with guardrails).
- **F9** No user-facing string exposes provider/chain/PIN/SDK/session internals.

### 3.2 Non-functional

- **N1 Security:** enumeration-safe, CSRF-protected, brute-force-throttled,
  fixation-safe sessions, secrets isolated. (§10)
- **N2 Reliability:** every transient failure is retryable and self-healing; no
  state strands a user.
- **N3 Performance:** code request < 1s p95; verify+provision < 3s p95 (mock
  instant; live Circle dominated by their API).
- **N4 Accessibility:** WCAG AA; full keyboard; OTP autofill on mobile.
- **N5 Privacy/Legal:** GDPR Art. 6/7/15/16/17/20 supported. (§11)
- **N6 Observability:** every state transition + failure is measurable. (§14)

---

## 4. Identity & custody model

### 4.1 Identity

- **Identity = a verified email.** `users.id` (UUID) is the durable key.
- Sessions, wallet, consent, portfolios all hang off `users.id`.
- Email is mutable (rectification, §11.5) without changing `users.id`.

### 4.2 Custody — Phase 1 (developer-controlled)

- Backend creates and controls an **SCA** wallet per user via Circle
  developer-controlled APIs; signing uses the **entity secret** (§8, §10.7).
- No PIN, no browser SDK, no challenge, no polling.
- `custody_model = 'circle_developer'` on the user row.

### 4.3 Custody — Phase 2 (non-custodial, sketch)

- From Settings → "Take self-custody": create a Circle **Modular** wallet with a
  **passkey** (WebAuthn) primary signer + an **EOA recovery key** (mnemonic) as
  a second signer; recover via `executeRecovery()` on device loss.
- Same `users.id`; `custody_model → 'external'`. Email login unchanged.
- Agent execution for self-custody users becomes "user signs the plan" or a
  scoped session key — a product change scoped with Phase 2.
- **Verify** Arc/Base coverage for Modular Wallets + SCA Paymaster first.

---

## 5. State machine

### 5.1 Server account status (`users.account_status`)

Two durable states. No "unconfirmed user" row exists (the user row is created
only at successful verify, §9), so there is no orphan cleanup.

| State            | Meaning                                                           |
| ---------------- | ----------------------------------------------------------------- |
| `pending_wallet` | email verified, wallet not yet provisioned (provider slow/failed) |
| `active`         | wallet provisioned and ready                                      |

**Transition table** (server):

| #   | From                      | Event                           | Guard                          | Action                                        | To                                              |
| --- | ------------------------- | ------------------------------- | ------------------------------ | --------------------------------------------- | ----------------------------------------------- |
| T1  | _(none)_                  | verify OK, email unknown        | consent.tos && consent.privacy | upsert user; record consent; provision wallet | `active` (or `pending_wallet` on provider fail) |
| T2  | _(none)_                  | verify OK, email known          | —                              | restore; (update marketing pref)              | unchanged (`active`/`pending_wallet`)           |
| T3  | `pending_wallet`          | verify OK / `GET /auth/session` | —                              | retry provision (idempotent)                  | `active` on success, else `pending_wallet`      |
| T4  | `active`                  | logout                          | —                              | revoke session                                | `active` (account persists)                     |
| T5  | `active`/`pending_wallet` | `POST /account/delete`          | wallet balance == 0            | request erasure                               | (erasure lifecycle, §11.4)                      |

### 5.2 Session lifecycle

- Created on every successful `verify`; **session id rotated** each time
  (anti-fixation). Backed by `auth_sessions` (revocable).
- States: `active` → `revoked` (logout) / `expired` (TTL) / `superseded` (new
  login rotates). Any non-active session → `401` → client routes to Continue.
- Idle + absolute timeouts enforced server-side (§10.4).

### 5.3 Client UI states

`signed_out` → `email_entry` → `code_sent` → `verifying` →
(`provisioning` rare) → `in_app`. Plus `error` overlays per screen (§12, §13).
The client **never** branches on new-vs-returning.

### 5.4 The ten scenarios → behavior

| #   | Scenario                             | Behavior                                                                      |
| --- | ------------------------------------ | ----------------------------------------------------------------------------- |
| 1   | new email, no user                   | T1 → app                                                                      |
| 2   | email verified, wallet not created   | `pending_wallet`; T3 self-heals                                               |
| 3   | user exists, wallet incomplete       | T3 idempotent re-provision; never a dead-end                                  |
| 4   | user exists, wallet ready            | T2 → app                                                                      |
| 5   | previous challenge expired           | code expired → user re-enters email for a fresh code (§13); account untouched |
| 6   | wants a different email              | "Use a different email" → resets to Continue                                  |
| 7   | logout then login                    | T4 then T1/T2; fresh session                                                  |
| 8   | returning user on the "signup" entry | identical; server signs in                                                    |
| 9   | new user on the "login" entry        | identical; server creates                                                     |
| 10  | provider failure at provisioning     | `pending_wallet` + "Finishing…" + retry; app degraded until ready             |

---

## 6. End-to-end sequences

```
NEW SIGNUP (happy)
Client                         API                         Circle (dev-controlled)
  │ POST /auth/email/start ───►│ rate-limit, mint code,
  │                            │ store hash, send email
  │◄── 200 {challengeId,…} ────│
  │ POST /auth/email/verify ──►│ verify code (single-use)
  │   {code,consent}           │ upsert user, record consent
  │                            │ create wallet ───────────────► POST .../wallets (SCA, all supported testnet routes)
  │                            │◄────────────────────────────── {networks:[arc,base,eth,arb,avax]}
  │                            │ status=active, rotate session,
  │◄── 200 {status,user,wallet}│ Set-Cookie: __Host-…
  │ (app opens)                │

RETURNING LOGIN: identical calls; verify finds user; no second wallet.

RESUME HALF-COMPLETE
  │ GET /auth/session ────────►│ user is pending_wallet → retry provision (idempotent)
  │◄── 200 {accountStatus,…} ──│ → active

PROVIDER FAILURE
  │ POST /auth/email/verify ──►│ create wallet ──► Circle 5xx
  │◄── 200 {status:"provisioning", wallet:null}
  │ (UI: "Finishing…")         │
  │ GET /auth/session (retry) ►│ retry provision … → active

LOGOUT / RELOGIN
  │ POST /auth/logout ────────►│ revoke session, clear cookie ── 204
  │ POST /auth/email/start … ─►│ (fresh code → fresh session)

DELETE ACCOUNT
  │ POST /account/delete ─────►│ guard: balance==0? if not → 409
  │◄── 202 {deletionRequestedAt}│ revoke sessions; schedule scrub
  │   …grace window…           │ anonymize PII; retain tax/AML (anonymized)
```

---

## 7. API contract (detailed)

### 7.1 Conventions

- Base path: existing API origin (separate from web origin). JSON only.
- **Auth:** session cookie (§10.2). Authed endpoints reject missing/invalid with
  `401`.
- **CSRF:** state-changing requests must send header `X-Aegis-Request: 1` (§10.3).
- **Error envelope (all non-2xx):**
  ```json
  {
    "error": {
      "code": "code_expired",
      "message": "<generic, user-safe>",
      "retryAfter": 30
    }
  }
  ```
  `code` is a stable machine string (§13); `message` is safe to show; optional
  `retryAfter` (seconds) on `429`.
- **Rate-limit headers:** `Retry-After`, `X-RateLimit-Remaining` on throttled
  responses.
- **Idempotency:** `verify` and provisioning are idempotent by design (§8.3).

### 7.2 `POST /auth/email/start`

Begin (or resume) auth. **Enumeration-safe: identical response for known/unknown
emails.**

- Auth: none. CSRF: required.
- Request: `{ "email": "a@b.com" }`
- 200: `{ "challengeId": "uuid", "expiresAt": "ISO-8601", "resendInSeconds": 30 }`
- Behavior: validate email format; apply rate limits (§10.4); invalidate prior
  live codes for this email; mint a 6-digit code, store HMAC hash + `expiresAt`
  (+10 min); send email. In real mode the code is **never** in the body.
- Errors: `400 invalid_email`; `429 rate_limited`. Email-send failures do **not**
  change the response (logged internally; surfaced only later, §13).

### 7.3 `POST /auth/email/verify`

Verify the code; create-or-restore; provision wallet; open a session.

- Auth: none. CSRF: required. Sets session cookie on success.
- Request:
  ```json
  {
    "challengeId": "uuid",
    "code": "933261",
    "consent": {
      "tos": true,
      "privacy": true,
      "tosVersion": "2026-05",
      "privacyVersion": "2026-05",
      "marketingOptIn": false
    }
  }
  ```
- 200:
  ```json
  {
    "status": "active", // | "provisioning"
    "user": {
      "id": "uuid",
      "email": "a@b.com",
      "riskTolerance": "moderate",
      "accountStatus": "active"
    },
    "wallet": {
      "walletId": "uuid",
      "arcAddress": "0x…",
      "baseAddress": "0x…",
      "networks": [
        {
          "blockchain": "ARC-TESTNET",
          "walletId": "uuid",
          "address": "0x…",
          "accountType": "SCA",
          "state": "LIVE"
        },
        {
          "blockchain": "BASE-SEPOLIA",
          "walletId": "uuid",
          "address": "0x…",
          "accountType": "SCA",
          "state": "LIVE"
        }
      ]
    }
  } // null if provisioning
  ```
- Behavior: constant-time HMAC compare; single-use (consume atomically); on
  unknown email → require `consent.tos && consent.privacy` (else
  `400 consent_required`), upsert user, store consent versions + `consented_at`,
  provision wallet (§8); on known email → restore, update `marketingOptIn` if
  present; **rotate session id**; return state.
- Idempotent: re-submitting a consumed code while a live session exists returns
  current state, never double-provisions.
- Errors: `400 code_invalid | code_expired | code_used | consent_required`;
  `429 too_many_attempts` (after 3 wrong attempts the code is invalidated).

### 7.4 `POST /auth/email/resend`

- Auth: none. CSRF: required.
- Request: `{ "challengeId":"uuid" }` → 200 `{ "expiresAt", "resendInSeconds" }`
- Honors cooldown; `429 resend_cooldown` if too soon.

### 7.5 `GET /auth/session` _(replaces `/auth/me` + `/auth/wallet/status`)_

- Auth: cookie required.
- 200: `{ "user":{…}, "wallet":{…|null}, "accountStatus":"active|pending_wallet" }`
- Behavior: if `pending_wallet`, **retry provisioning (idempotent) before
  responding** so returning half-finished accounts self-heal.
- Errors: `401 session_invalid` (missing/expired/revoked).

### 7.6 `POST /auth/logout`

- Auth: cookie. CSRF: required.
- 204; revoke the active `auth_sessions` row; clear cookie (Max-Age=0). Always
  succeeds from the client's perspective — no "retry logout" state.

### 7.7 `POST /account/export` _(GDPR Art. 15/20)_

- Auth: cookie. CSRF: required. → 202
  `{ "status":"queued", "deliveryEmail":"a@b.com", "expiresAt":"ISO-8601" }`
- Generates a machine-readable archive (§11.3); delivered as a signed, expiring
  link by email. Rate-limited on successful deliveries (e.g. 1/day), so a mail
  provider failure does not consume the user's retry quota.

### 7.8 `POST /account/delete` _(GDPR Art. 17)_

- Auth: cookie. CSRF: required.
- Request: `{ "confirm": true }`
- 202: `{ "deletionRequestedAt":"ISO-8601", "completesAt":"ISO-8601" }`
- 409 `funds_present`: `{ "error":{ "code":"funds_present", "message":"Move your funds out before closing your account." } }`
- Behavior: §11.4 (funds guard → revoke sessions → grace window → anonymize,
  retaining legally required records).

### 7.9 Removed endpoints

`/auth/wallet/readiness`, `/auth/wallet/create`, `/auth/wallet/login`,
`/auth/wallet/status`. No `bundle`/`UserTokenBundle` ever returned.

---

## 8. Wallet provisioning spec (developer-controlled)

### 8.1 One-time setup

- Create a **wallet set** once; persist its id (`users.wallet_set_id` references
  the active set, or a singleton config). Entity secret registered with Circle;
  recovery file stored offline (§10.7).
- Local setup command: `cargo run --bin circle_wallet_setup -- check|list|create`
  from `apps/api/`. `create --write-env-local` writes only
  `CIRCLE_WALLET_SET_ID`; the entity secret must already be generated and
  registered in Circle.
- Entity-secret registration: run
  `cargo run --bin circle_wallet_setup -- entity-ciphertext --generate --write-env-local`,
  paste the printed ciphertext into Circle's registration form, then save
  Circle's recovery file offline. The raw entity secret stays in `.env.local`
  and is never printed.

### 8.2 Per-user provisioning (inside `verify` / `session` self-heal)

1. Ensure Circle user/account scaffolding for `users.id` (idempotent).
2. `create_wallet(user_id)` → **SCA** on **ARC-TESTNET, BASE-SEPOLIA,
   ETH-SEPOLIA, ARB-SEPOLIA, and AVAX-FUJI**, signed with entity-secret
   ciphertext (single-use, replay-safe).
3. Read back all supported chain addresses; **upsert one `user_wallet_networks` row per
   chain** (source of truth, §9.2); set `account_status = active`. Legacy
   `users.arc_address`/`base_address` are projection-only and were nulled for
   pre-cutover rows by migration `0028`.

### 8.3 Idempotency & retry

- If `wallet_id` already set → return it (no-op).
- Use a stable idempotency key per user so retries don't double-create.
- On transient Circle failure: leave `pending_wallet`, return
  `status:"provisioning"`. Retried by `GET /auth/session` and a background
  reconciler (bounded exponential backoff). No client polling loop.

### 8.4 Gas & chains

- SCA enables **Paymaster/Gas-Station** so users never hold native gas. On Arc,
  gas is USDC-native. Required for CCTP V2 Hooks.

### 8.5 Failure semantics

| Failure              | Account          | User sees                              | Recovery                                   |
| -------------------- | ---------------- | -------------------------------------- | ------------------------------------------ |
| Circle 5xx / timeout | `pending_wallet` | "Finishing your account…"              | auto-retry on next `session`, + reconciler |
| Partial (one chain)  | `pending_wallet` | same                                   | re-fetch until both present                |
| Persistent           | `pending_wallet` | "Taking longer than usual — try again" | manual retry button hits `session`         |

---

## 9. Data model & migrations

### 9.1 `users` (additions)

| Column                  | Type                                     | Notes                                                  |
| ----------------------- | ---------------------------------------- | ------------------------------------------------------ |
| `account_status`        | TEXT NOT NULL default `active`           | CHECK in (`pending_wallet`,`active`)                   |
| `custody_model`         | TEXT NOT NULL default `circle_developer` | CHECK in (`circle_developer`,`circle_user`,`external`) |
| `wallet_set_id`         | TEXT                                     | dev-controlled wallet set                              |
| `tos_version`           | TEXT                                     | consent                                                |
| `privacy_version`       | TEXT                                     | consent                                                |
| `consented_at`          | TIMESTAMPTZ                              | consent                                                |
| `marketing_opt_in`      | BOOLEAN NOT NULL default FALSE           | separate opt-in                                        |
| `deletion_requested_at` | TIMESTAMPTZ                              | erasure soft window                                    |
| `anonymized_at`         | TIMESTAMPTZ                              | erasure final                                          |

(unchanged, nullable: `wallet_id`, `arc_address`, `base_address`.)

### 9.2 `user_wallet_networks`

Circle represents a developer-controlled account wallet as one row per
blockchain, even when EVM networks share the same SCA address. Aegis persists
those rows as network routes under one user wallet so more chains and tokens can
be added without adding columns to `users`.

| Column             | Type | Notes                                 |
| ------------------ | ---- | ------------------------------------- |
| `user_id`          | UUID | references `users(id)`                |
| `blockchain`       | TEXT | Circle chain code, e.g. `ARC-TESTNET` |
| `circle_wallet_id` | TEXT | Circle per-network wallet id          |
| `address`          | TEXT | network address                       |
| `account_type`     | TEXT | `SCA` for Phase 1                     |
| `wallet_set_id`    | TEXT | configured developer wallet set       |
| `state`            | TEXT | Circle state, usually `LIVE`          |

Primary key: `(user_id, blockchain)`. Auth/session readiness, Gateway,
billing, tax, execution, diary, account export, and deletion guards read this
route table as the wallet source of truth. The legacy `users.arc_address` and
`users.base_address` columns remain only as a temporary compatibility projection
for older rows until a final column-drop migration.

### 9.3 Existing tables (kept)

- `wallet_auth_codes` — 6-digit, HMAC-hashed, `expires_at`, `attempts`,
  single-use `consumed_at`. **Drop `intent` from the contract** (column nullable
  during migration, drop in `0026`).
- `auth_sessions` — `id` (jti), `user_id`, `expires_at`, `revoked_at`,
  `created_at`, `last_seen_at`. Drives revocation + session rotation.

### 9.4 Migration SQL (originally `0025_unified_auth.sql`)

> The 0001–0039 migration history has since been squashed into a single
> `0001_baseline.sql`; the DDL below is kept as the design record for the auth
> schema and now lives in that baseline.

```sql
ALTER TABLE users
  ADD COLUMN account_status TEXT NOT NULL DEFAULT 'active'
    CHECK (account_status IN ('pending_wallet','active')),
  ADD COLUMN custody_model TEXT NOT NULL DEFAULT 'circle_developer'
    CHECK (custody_model IN ('circle_developer','circle_user','external')),
  ADD COLUMN wallet_set_id TEXT,
  ADD COLUMN tos_version TEXT,
  ADD COLUMN privacy_version TEXT,
  ADD COLUMN consented_at TIMESTAMPTZ,
  ADD COLUMN marketing_opt_in BOOLEAN NOT NULL DEFAULT FALSE,
  ADD COLUMN deletion_requested_at TIMESTAMPTZ,
  ADD COLUMN anonymized_at TIMESTAMPTZ;
ALTER TABLE wallet_auth_codes ALTER COLUMN intent DROP NOT NULL;
-- index for the erasure/retention reconciler
CREATE INDEX users_deletion_pending_idx ON users (deletion_requested_at)
  WHERE deletion_requested_at IS NOT NULL AND anonymized_at IS NULL;
```

`0026_drop_auth_code_intent.sql` (post-cutover): `ALTER TABLE wallet_auth_codes DROP COLUMN intent;`

**Shipped migration set (as built, reconciled with code; all squashed into
`0001_baseline.sql`):** `0025_unified_auth`
(columns above + an `account_status='pending_wallet'` backfill),
`0026_drop_auth_code_intent`, `0027_auth_rate_limits` (per-IP throttle table,
hashed bucket ids — §10.4), `0028_clear_legacy_wallet_rows` (null pre-cutover
wallet rows so `/auth/session` self-heals — §8.3), `0029_user_wallet_networks`
(the §9.2 routes table + backfill), `0031_account_export_jobs` (queued export
rows behind `POST /account/export` — §7.7/§11.3), and
`0032_wallet_provision_retries` (persisted bounded retry state for the background
wallet reconciler — §8.3/§8.5).

---

## 10. Security spec & threat model

### 10.1 Threat model

| Threat                   | Mitigation                                                                 |
| ------------------------ | -------------------------------------------------------------------------- |
| Account enumeration      | identical `start` response + timing for known/unknown; no `intent` 401     |
| Code brute force         | 6 digits, 3 attempts/code then invalidate, per-email + per-IP rate limits  |
| Session fixation         | rotate session id on every verify                                          |
| Session theft (XSS)      | `HttpOnly` cookie; opaque session id (not a portable JWT)                  |
| CSRF (cross-origin API)  | custom header `X-Aegis-Request` + strict CORS allowlist + `SameSite`       |
| Replay (Circle signing)  | single-use entity-secret ciphertext                                        |
| Entity-secret compromise | HSM/secrets manager, offline recovery file, least privilege, rotation plan |
| PII leakage in logs      | redact email/PII; never log codes or tokens                                |

### 10.2 Session cookie

| Attribute  | Prod                                             | Local dev             |
| ---------- | ------------------------------------------------ | --------------------- |
| name       | `__Host-aegis_session`                           | `aegis_session`       |
| `HttpOnly` | yes                                              | yes                   |
| `Secure`   | yes                                              | no (http://localhost) |
| `SameSite` | `Lax`                                            | `Lax`                 |
| `Path`     | `/`                                              | `/`                   |
| value      | **opaque session id** (recommended) over raw JWT | same                  |
| Max-Age    | = session absolute TTL                           | same                  |

### 10.3 CSRF & CORS

- CSRF: require `X-Aegis-Request: 1` on all state-changing routes; only
  same-origin JS can set it. Reject otherwise (`403 csrf_failed`).
- CORS: explicit origin allowlist + `Access-Control-Allow-Credentials: true`
  (wildcard illegal with credentials); add `X-Aegis-Request` to allowed headers.

### 10.4 Rate limits & timeouts

| Surface                    | Limit                        |
| -------------------------- | ---------------------------- |
| `start` per email          | ≤ 3 / 10 min, ≤ 10 / hr      |
| `start`/`verify` per IP    | bounded (e.g. ≤ 20 / 10 min) |
| `verify` attempts per code | 3, then invalidate           |
| `resend` cooldown          | 30 s                         |
| code expiry                | 10 min (NIST ≤10)            |
| session idle timeout       | 30 min (server-enforced)     |
| session absolute TTL       | configurable (e.g. 24 h)     |

### 10.5 Enumeration

`start` always `200` + generic copy; pre-verification responses reveal nothing
about account existence; remove all "this email is new" copy.

### 10.6 No mocks/dev shortcuts in real mode

`devCode` is `None` whenever `MOCK_CIRCLE=false`; `readiness` (internal flags) is
removed from the client surface entirely.

### 10.7 Entity-secret management

Stored only in an HSM/secrets manager; never in VCS or client; recovery file
kept offline; documented blast radius; rotation runbook.

---

## 11. GDPR & legal spec

### 11.1 PII inventory (auth/account surface)

| Data                  | Purpose                         | Lawful basis        | Retention                                               |
| --------------------- | ------------------------------- | ------------------- | ------------------------------------------------------- |
| email                 | identity, login, notifications  | contract            | life of account; anonymized on erasure (unless tax/AML) |
| consent records       | legal proof                     | legal obligation    | retained per statute                                    |
| session metadata      | security                        | legitimate interest | short (rolling)                                         |
| wallet addresses      | provide service                 | contract            | tax/AML retention may apply                             |
| tax lots / tx history | provide service + tax reporting | legal obligation    | statutory retention (anonymized on erasure)             |
| marketing opt-in      | consented comms                 | consent             | until withdrawn                                         |

### 11.2 Consent (Art. 6/7)

- Captured at **first verify for a new email**: ToS + Privacy via clickwrap
  ("By continuing, you agree…") + a **separate, unticked** marketing checkbox.
- Stored: `tos_version`, `privacy_version`, `consented_at`, `marketing_opt_in`.
- Re-prompt on version bump before allowing continue. Returning users re-affirm
  silently (shown to all → no enumeration).

### 11.3 Data export (Art. 15/20)

- `POST /account/export` → queued archive row (JSON), emailed signed expiring
  backend download link. The email must never attach the archive directly.
- Rate limiting is based on delivered export links, not attempted requests; a
  failed mail-provider call must be retryable after the provider is fixed.
- Contents: profile, consent history, the account wallet network routes,
  portfolios, allocations, agent decisions, rebalance events, tax lots, referrals.
  Excludes secrets/other users' data and never reads legacy wallet columns as the
  source of truth.

### 11.4 Erasure (Art. 17) — procedure

1. **Funds guard:** if wallet balance > 0 → `409 funds_present`; instruct
   withdrawal first.
2. Set `deletion_requested_at`; **revoke all sessions**; disable login.
3. Grace window (e.g. 7 days, configurable) for accidental requests.
4. **Anonymize**: null/scramble email + PII; keep `users.id` referential
   integrity; set `anonymized_at`.
5. **Retain** statutory records (1099-DA basis, AML/tx logs) in
   pseudonymized/anonymized form. Erasure = "as far as the law permits,"
   documented.

- Reconciler job processes the `users_deletion_pending_idx` queue.

### 11.5 Rectification (Art. 16)

- Edit email (re-verify the new address) + profile from Settings; `users.id`
  unchanged.

### 11.6 Financial-regulatory flag (larger than GDPR)

Custodial custody (Phase 1) likely triggers **KYC/AML + money-transmission /
VASP** obligations. This is a **legal/compliance workstream and a go-live
blocker**, not an engineering detail. Strong reason to engage counsel before
custodial launch and/or prioritize Phase-2 non-custodial (which reduces this
exposure). Out of scope to _implement_ here; in scope to flag and gate.

---

## 12. Frontend spec & copy deck

### 12.1 Routes

- `/login` and `/signup` both resolve to **one** "Continue with email" screen.
- Authed app routes use `GET /auth/session` as the gate (self-heals
  `pending_wallet`).

### 12.2 Screens & states

**S1 Continue** — field: Email; button: Continue (disabled until valid email).
**S2 Enter code** — 6-digit input (`inputmode=numeric`, `autocomplete=one-time-code`);
Continue; consent microcopy + marketing checkbox; Resend (cooldown); "Use a
different email". **S3 Finishing… (rare)** — spinner; after timeout, "Try again".
Each screen: default / loading / error states (copy in §12.3, §13).

### 12.3 Copy deck (final, jargon-free)

| Location                | Copy                                                          |
| ----------------------- | ------------------------------------------------------------- |
| S1 title                | **Continue with email**                                       |
| S1 sub                  | We'll email you a 6-digit code.                               |
| S1 field label          | Email                                                         |
| S1 button               | Continue                                                      |
| S2 title                | Enter the code we emailed you                                 |
| S2 sub                  | Sent to {email}.                                              |
| S2 field label          | 6-digit code                                                  |
| S2 button               | Continue                                                      |
| S2 consent              | By continuing, you agree to our [Terms] and [Privacy Policy]. |
| S2 marketing (unticked) | Email me product updates.                                     |
| S2 resend               | Resend code ({n}s)                                            |
| S2 change               | Use a different email                                         |
| S3 title                | Setting up your account…                                      |
| S3 slow                 | This is taking longer than usual.                             |
| S3 retry                | Try again                                                     |
| logged-out banner       | Signed out. Enter your email to continue.                     |

**Banned strings** (must not appear anywhere user-facing): "Circle", "Arc",
"Base", "PIN", "challenge", "provider", "session token", "wallet provisioning",
"SESSION VERIFIED", "this email is new", "Mock dev code".

### 12.4 Accessibility & mobile

- Labels + `aria-live` on errors; focus moves to the code field on S2; visible
  focus rings; AA contrast.
- Mobile: numeric keypad, OS OTP autofill, no input zoom; single-column layout
  (verified at 390×844).

---

## 13. Master error catalog

| `code`              | HTTP                      | When                               | User-facing copy                                                      |
| ------------------- | ------------------------- | ---------------------------------- | --------------------------------------------------------------------- |
| `invalid_email`     | 400                       | bad email format                   | Enter a valid email address.                                          |
| `code_invalid`      | 400                       | wrong code                         | That code didn't match. Check it or request a new one.                |
| `code_expired`      | 400                       | code > 10 min                      | That code expired. Enter your email to get a new one.                 |
| `code_used`         | 400                       | already consumed                   | That code was already used. Enter your email to get a new one.        |
| `too_many_attempts` | 429                       | >3 wrong                           | Too many tries. Request a new code.                                   |
| `consent_required`  | 400                       | new user, ToS/Privacy not accepted | Please accept the Terms and Privacy Policy to continue.               |
| `rate_limited`      | 429                       | code requests exceeded             | Too many requests. Try again shortly.                                 |
| `resend_cooldown`   | 429                       | resend too soon                    | You can request a new code in {n}s.                                   |
| `session_invalid`   | 401                       | missing/expired/revoked            | Your session expired. Enter your email to continue.                   |
| `csrf_failed`       | 403                       | missing/invalid CSRF header        | Something went wrong. Refresh and try again.                          |
| `email_send_failed` | (200 to client; internal) | provider error                     | (Only after a dependent step:) We couldn't send your code. Try again. |
| `provisioning`      | 200                       | wallet not ready                   | Setting up your account…                                              |
| `funds_present`     | 409                       | delete with balance                | Move your funds out before closing your account.                      |

Generic fallback for unexpected 5xx: "Something went wrong on our end. Try
again." Internal cause is logged, never shown.

---

## 14. Observability & analytics

- **Events:** `auth.code_requested`, `auth.code_verified`, `auth.signup_created`,
  `auth.login_restored`, `auth.provision_started/succeeded/failed`,
  `auth.session_rotated`, `auth.logout`, `account.export_requested`,
  `account.delete_requested/completed`. (No PII/codes in payloads.)
- **Metrics:** verify success rate, provision success rate + latency, time-to-app
  (start→in_app) p50/p95, resend rate, rate-limit hits, `pending_wallet` dwell
  time, enumeration-probe rate.
- **Alerts:** provision failure rate spike, code-send failure spike, abnormal
  start volume per IP.

---

## 15. Migration / rollout plan

Single cutover on one branch (no dual-path shim — repo convention). Order:

1. **Backend provider:** add developer-controlled provider (create wallet set;
   `create_wallet` SCA ARC+BASE; entity-secret ciphertext). Remove
   `with_initialize_challenge` + `/v1/w3s/user/initialize`.
2. **Backend endpoints:** add `email/start|verify|resend`, `session`, account
   `export|delete`; drop `create|login|readiness|status` and the `bundle`;
   add CSRF header check + session rotation + opaque session id.
3. **DB:** apply `0025` (§9.3).
4. **Frontend:** collapse `CreateWalletCard` to S1/S2/S3; delete challenge +
   polling + readiness gate + `@circle-fin/w3s-pw-web-sdk` dependency; point
   `/login` + `/signup` at one route; auth gate uses `GET /auth/session`; ship
   the §12.3 copy deck; add consent UI; add Settings export/delete.
5. **Data:** new accounts get developer-controlled wallets. **Confirm whether any
   production user-controlled wallets hold real funds** (§18-3); if so, plan
   migration vs re-onboard; legacy addresses still receive but won't sign.
6. **Rollback:** revert branch; `0025` is additive (safe to keep); only the
   `intent` drop needs the `0026` follow-up.

**Cutover checklist:** entity secret provisioned + recovery file stored; wallet
set created via `circle_wallet_setup`; Arc/Base dev-controlled SCA confirmed;
CORS/CSRF/cookie config set per env (`SESSION_COOKIE_SECURE=true` with a
`__Host-` cookie name in production); rate limits configured; consent doc
versions published; export/delete tested; legal sign-off on custodial posture
(§11.6).

---

## 16. Test plan

| Area              | Case                                         | Type        | Expected                                                          |
| ----------------- | -------------------------------------------- | ----------- | ----------------------------------------------------------------- |
| Signup            | unknown email → code → verify                | e2e         | user created, wallet `active`, app opens                          |
| Login             | known email → code → verify                  | e2e         | session restored, no second wallet                                |
| Unified entry     | new email on `/login`; existing on `/signup` | e2e         | identical happy path, no error                                    |
| Logout            | logout → 204                                 | integration | cookie cleared, session revoked, `session`→401                    |
| Refresh           | reload with valid cookie                     | e2e         | `session` returns user+wallet, no re-auth                         |
| Half-complete     | kill after verify, before wallet             | integration | `pending_wallet`; next `session` → `active`                       |
| Idempotent verify | same code twice / double-click               | integration | one user, one wallet, second no-ops                               |
| Expired code      | wait >10 min                                 | integration | `code_expired`, user is sent back to email entry for a fresh code |
| Wrong code        | 3 bad attempts                               | integration | code invalidated, `too_many_attempts`                             |
| Resend cooldown   | resend <30s                                  | integration | `resend_cooldown`                                                 |
| Rate limit        | exceed per-email/per-IP                      | integration | `rate_limited` + `Retry-After`                                    |
| Provider fail     | force Circle 5xx                             | integration | `provisioning`, retry heals, no crash                             |
| Session expiry    | revoke/expire mid-use                        | integration | `401` → "session expired" → Continue                              |
| Enumeration       | known vs unknown email on `start`            | security    | identical status + body + ~timing                                 |
| CSRF              | state-changing request w/o header            | security    | `403 csrf_failed`                                                 |
| Fixation          | session id before vs after verify            | security    | id rotates                                                        |
| Consent           | new signup                                   | integration | tos/privacy required, versions stored, marketing default off      |
| Re-consent        | bump version                                 | integration | returning user re-prompted                                        |
| Export            | `POST /account/export`                       | integration | archive queued; rate-limited; missing CSRF rejected               |
| Erasure guard     | delete with balance                          | integration | `funds_present`                                                   |
| Erasure           | delete empty account                         | integration | sessions revoked, PII scrubbed, tax/AML retained anonymized       |
| Marketing         | opt-in unticked then toggled                 | integration | digest only after explicit opt-in                                 |
| Mobile UX         | iOS/Android OTP autofill, paste, viewport    | e2e         | autofill works, no zoom/jank                                      |
| Copy audit        | scan all auth screens                        | manual      | no banned strings (§12.3)                                         |

---

## 17. Acceptance criteria (definition of done)

- A new user reaches the app in **≤3 screens** with **no** PIN/challenge/polling.
- A returning user signs in via the **same** screen; no second wallet.
- A half-finished account **self-heals** with no dead-end.
- Logout + immediate re-login works with no "retry logout" state.
- `start` is **enumeration-safe** (verified by test).
- CSRF, session rotation, rate limits, cookie attributes per §10 (verified).
- Consent captured + versioned; export + delete (with funds guard + retention)
  function per §11 (verified).
- No banned string appears in any auth surface (§12.3).
- Telemetry (§14) emits for every transition + failure.

---

## 18. Open questions

1. **Custodial posture** (Phase 1) — approved? _(Gates everything.)_
2. **Arc support** — Circle developer-controlled SCA + Paymaster on ARC-TESTNET?
3. **Existing wallets** — any production user-controlled wallets with real funds
   needing migration vs re-onboard?
4. **Cookie value** — opaque session id now, or defer (auth_sessions supports it)?
5. **Legal/regulatory** — KYC/AML + money-transmitter/VASP scope; GDPR retention
   windows + jurisdictions; confirm with counsel before custodial go-live.
6. **Erasure grace window** length; **export** rate limit + format details.

---

## 19. Glossary

- **SCA** — smart contract account (vs EOA); needed for Paymaster + CCTP Hooks.
- **Developer-controlled wallet** — Circle wallet whose keys the app controls via
  an entity secret; no user PIN/SDK.
- **User-controlled wallet** — Circle wallet requiring user PIN/passkey per
  sensitive action (today's model; being removed).
- **Modular wallet** — ERC-6900/4337 Circle wallet supporting passkey signers +
  recovery (Phase 2 non-custodial path).
- **Entity secret** — the app's signing secret for developer-controlled wallets.
- **Enumeration-safe** — responses don't reveal whether an email has an account.
- **Clickwrap** — consent captured by an affirmative action (the Continue button)
  with visible Terms/Privacy links.
