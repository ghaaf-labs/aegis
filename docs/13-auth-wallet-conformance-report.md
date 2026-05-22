# 13 — Auth & Wallet Onboarding — Conformance Report

> **Date:** 2026-05-22 · **Audited against:** [`12-auth-wallet-onboarding.md`](./12-auth-wallet-onboarding.md)
> · **Method:** read-only audit of the shipped code across 5 parallel passes
> (backend auth, account/GDPR, provider/security, frontend, tests). Evidence is
> `file:line` from the current tree. No code was changed.

## Verdict

The cutover is **real, faithful, and high quality.** The unified
`Continue with email` flow, developer-controlled wallet provisioning, opaque
session + rotation, CSRF, rate limiting, and the account export/delete paths are
implemented and (by code-read) behave as specified. The old user-controlled
provider, the PIN/SDK ceremony, and `/auth/wallet/{create,login,readiness,status}`
are fully gone.

Remaining gaps are **not** in the auth core. They are: (1) GDPR completeness
(export contents, re-consent, rectification), (2) one dev-only copy render, (3) a
missing background reconciler, and (4) **automated test coverage of the
security-critical behaviors** — the controls exist but are largely unverified by
tests, so they're regression-prone.

### Scorecard

| Area                              | Conformance | Headline                                                                                                                             |
| --------------------------------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Backend auth (§7/§10/§13)         | ✅ ~Full    | enumeration-safe, opaque session + rotation, CSRF global, 3-attempt limit, idempotent, self-heal — all present                       |
| Provider + security (§8/§9.2/§10) | ✅ ~Full    | dev-controlled SCA, RSA-OAEP single-use ciphertext, network-routes source of truth, hashed per-email+per-IP limits, `__Host-` cookie |
| Account / GDPR (§7.7-7.8/§11)     | ⚠️ Partial  | export/delete/erasure are real; **export incomplete, no re-consent, no rectification**                                               |
| Frontend (§12)                    | ✅ Mostly   | unified entry, SDK removed, consent UI + Settings export/delete present; one dev-only copy render + a11y nits                        |
| Tests (§16)                       | ❌ Thin     | **no endpoint integration tests**; security ACs unverified; mock-mode only                                                           |

---

## Consolidated findings (severity-ranked)

Severity = impact on a **real custodial launch**. "Control exists" means a
code-read confirmed correct behavior; the gap is test coverage.

### MAJOR — fix before a real/EU launch

| ID     | Finding                                                                        | Spec      | Evidence                                                              | Fix                                                                                                                                     |
| ------ | ------------------------------------------------------------------------------ | --------- | --------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| **G1** | Data export **omits tax lots** (`cost_basis_lots`)                             | §11.3     | `account/handlers.rs:35-132` (absent); table `0003`                   | add a scoped `cost_basis_lots WHERE user_id` subquery to the archive                                                                    |
| **G2** | Data export **omits referrals**                                                | §11.3     | `account/handlers.rs:35-132`; table `0005`                            | add scoped `referrals` to the archive                                                                                                   |
| **G3** | **No consent re-prompt on ToS/Privacy version bump**                           | §11.2     | `service.rs:757-759` (returning user only updates `marketing_opt_in`) | compare stored vs current version; force re-accept before continue                                                                      |
| **G4** | **No email-rectification endpoint** (Art. 16)                                  | §11.5     | no route in `router.rs`; grep empty                                   | add `POST /account/email` (re-verify new address)                                                                                       |
| **T1** | No test: **session-id rotation on verify** (fixation control)                  | §10/§17   | none; control at `service.rs:978-1007`                                | integration test asserting jti changes per verify                                                                                       |
| **T2** | No test: **enumeration parity** (known vs unknown vs deletion-pending `start`) | §10.5/§17 | none; behavior at `service.rs:108-195`                                | integration test: identical status+body                                                                                                 |
| **T3** | No request-level test: **missing `X-Aegis-Request` → 403**                     | §10.3/§17 | only predicate unit-tested (`csrf.rs`)                                | router-level test hitting a state-changing route w/o header                                                                             |
| **T4** | No test: **delete with balance → 409 `funds_present`**                         | §11.4/§17 | only `balance_has_funds` helper tested                                | integration test on `/account/delete` with funded wallet                                                                                |
| **T5** | **No DB-backed integration tests for any `/auth/*` or `/account/*` endpoint**  | §16       | `apps/api/tests/` covers billing/tax/calibration only                 | stand up router + Postgres; cover rate-limit/cooldown enforcement, idempotent verify, self-heal, erasure execution, consent persistence |

### MINOR — polish / hardening

| ID     | Finding                                                                                           | Spec        | Evidence                                                 | Fix                                                                                                                                                                                                                                     |
| ------ | ------------------------------------------------------------------------------------------------- | ----------- | -------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **F1** | S2 renders **"Local code: {code}"** when `devCode` present                                        | §10.6/§12.3 | `email-auth-card.tsx:407-413`                            | **Not** prod-visible (backend returns `devCode` only in mock+localhost, `handlers.rs:77-92`), but reintroduces the banned dev-shortcut pattern; guard/remove client-side + add a test (current test only checks the old literal string) |
| **F2** | No focus-to-code-field on S2; error block uses `role="alert"` not `aria-live`                     | §12.4       | `email-auth-card.tsx:478-482`                            | move focus to code input on S1→S2; add `aria-live="assertive"`                                                                                                                                                                          |
| **F3** | Consent links `/policy#terms` / `#privacy` have **no matching anchors**                           | §12.3       | `email-auth-card.tsx:439,446`; `policy/page.tsx`         | add `id="terms"`/`id="privacy"` to the policy page                                                                                                                                                                                      |
| **F4** | S3 copy drift: `"..."` vs `"…"`; "Opening Aegis..." not in deck                                   | §12.3       | `email-auth-card.tsx:342,343,570`                        | align to copy deck (cosmetic)                                                                                                                                                                                                           |
| **P1** | **No background wallet-provisioning reconciler**                                                  | §8.3/§8.5   | only erasure reconciler at `router.rs:78`                | a user stuck `pending_wallet` who never returns isn't healed; add a reconciler OR soften the spec to "on-demand self-heal only"                                                                                                         |
| **G5** | Erasure grace window hardcoded 7d (spec says configurable)                                        | §11.4       | `account/erasure.rs:9`                                   | make env-configurable                                                                                                                                                                                                                   |
| **G6** | Funds guard returns 503 (fail-closed) in mock mode → untestable without live Circle               | §11.4       | `account/handlers.rs:311-315`                            | acceptable, but blocks automated funds-guard tests in CI; consider a mock balance hook                                                                                                                                                  |
| **B1** | `start` is response-identical but **timing is not constant** (slight oracle)                      | §10.1/§10.5 | `service.rs:268-272` (verify branches on user existence) | spec says "~timing"; optionally equalize work                                                                                                                                                                                           |
| **B2** | `Retry-After` / `retryAfter` surfacing depends on `normalize_error_response` (not in audit scope) | §7.1/§13    | `service.rs:85,220-223` encode `:{n}` in message         | confirm the normalizer parses `:{n}` into the header/field                                                                                                                                                                              |
| **D1** | `account_deletion_requested` (403) state is returned but **not in the §13 error catalog**         | §13         | `service.rs:399-401`                                     | add the code to the spec's catalog (post-verify, no enumeration risk)                                                                                                                                                                   |

> No **BLOCKER** code defects were found. The items labelled BLOCKER by the test
> pass (rotation, enumeration, CSRF 403, funds-guard) are **coverage** gaps — the
> underlying controls are implemented and verified by code-read; they are
> regression-prone without tests, hence MAJOR here.

---

## What's verified good (don't re-litigate)

- **Enumeration-safe `start`** — identical `WalletAuthCodeResponse` for
  known/unknown; no `intent` 401 (`handlers.rs:99-107`, `service.rs:108-195`).
- **Opaque session id + rotation** — cookie value is the `auth_sessions` UUID,
  not a JWT; prior live sessions revoked and a fresh id minted per verify
  (`service.rs:978-1007`, `auth.rs:103-105`).
- **CSRF** — `require_request_header` enforced as a global layer; `x-aegis-request`
  in CORS `allow_headers` (`csrf.rs:10-29`, `router.rs:282,301-304`).
- **Cookie** — `__Host-aegis_session` (prod) / `aegis_session` (local); validation
  couples `__Host-`↔`Secure` both directions (`config.rs:472-481`).
- **Rate limits** — per-email (3/10m, 10/hr) + per-IP (≤20/10m), **IP hashed**
  with SHA-256 (`service.rs:50-91,125-149,953-959`; table `0027`).
- **Provider** — dev-controlled SCA on Arc+Base under the wallet set, `refId =
users.id`, RSA-OAEP **single-use** ciphertext, idempotent create
  (`provider.rs:143-154,182-218,276-297`).
- **Network routes** — `user_wallet_networks` is the source of truth; gateway,
  tax, execution, export, delete, diary all read it; no consumer reads legacy
  `arc_address`/`base_address` (`service.rs:803-826` + consumer cites).
- **Export/delete are real** — signed expiring link (no attachment), rate-limited
  on _delivered_ links, funds-guard via Gateway, erasure reconciler anonymizes
  email + revokes sessions + sets `anonymized_at` (`account/handlers.rs:182-435`,
  `account/erasure.rs:9-78`).
- **Consent capture** — `tos_version`/`privacy_version`/`consented_at` stored on
  new-user insert; `consent_required` enforced when missing (`service.rs:736-784,940-947`).
- **Frontend cutover** — `@circle-fin/w3s-pw-web-sdk` removed (zero hits incl.
  lockfile); no polling/challenge; unified `/login`+`/signup`; client sends CSRF
  header + `credentials:include` and uses only the new endpoints (`api.ts:42,49,111-213`).

---

## Recommended remediation order

1. **GDPR completeness (G1, G2, G4, G3)** — required for a lawful EU launch
   (Art. 15/20 export, Art. 16 rectification, Art. 7 re-consent). Mostly additive.
2. **Security/behavior integration tests (T1–T5)** — the controls are correct
   today but unguarded; for a custodial app these are the highest-leverage tests.
   Stand up a router+Postgres harness in `apps/api/tests/`.
3. **Reliability (P1)** — add the provisioning reconciler or soften §8.3/§8.5.
4. **Frontend polish (F1–F4)** — remove the `Local code:` render, fix focus/
   `aria-live`, add policy anchors.
5. **Spec touch-ups (D1, B2, G5)** — add the `account_deletion_requested` code to
   §13; confirm `Retry-After` plumbing; make the grace window configurable.

---

## Method & coverage

Five parallel read-only audits, each grounded in specific spec sections:
backend auth (`wallet/handlers.rs`, `service.rs`, `models.rs`, `middleware/{auth,csrf}.rs`,
`router.rs`); account/GDPR (`account/*`, `0028`, `0031`); provider/security
(`provider.rs`, `wallet_routes.rs`, `config.rs`, `0027`, `0029`,
`circle_wallet_setup.rs`); frontend (`email-auth-card.tsx`, route pages,
`auth-gate.tsx`, `lib/api.ts`, `settings/*`, `package.json`); tests (Rust
`#[cfg(test)]`, Vitest `*.test.tsx`, Playwright `e2e/*`).

**Coverage caveat:** all passing automated tests run in **mock mode only**
(Playwright specs `test.skip()` unless `MOCK_CIRCLE=true` exposes `devCode`;
Vitest mocks the API client). The live-Circle provisioning path, real email
delivery, and `devCode=None` behavior have no automated coverage.
