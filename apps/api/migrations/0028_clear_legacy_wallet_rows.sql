-- ── 0028 — Clear pre-developer-wallet rows ───────────────────────────────
--
-- Accounts created before the docs/12 cutover can carry wallet ids/addresses
-- that do not belong to the configured Circle developer-controlled wallet set.
-- Treat them as incomplete so /auth/session self-heals by provisioning a fresh
-- developer-controlled SCA wallet.

UPDATE users
SET wallet_id = NULL,
    arc_address = NULL,
    base_address = NULL,
    account_status = 'pending_wallet',
    custody_model = 'circle_developer'
WHERE (wallet_set_id IS NULL OR wallet_set_id = '')
  AND (
    wallet_id IS NOT NULL
    OR arc_address IS NOT NULL
    OR base_address IS NOT NULL
  );
