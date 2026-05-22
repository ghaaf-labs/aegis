-- F-PEG-5 -- fail-closed peg-rule bounds.
--
-- Rules above $1 would fire continuously for healthy stable assets. Clamp and
-- pause any legacy rows before adding database constraints so the monitor never
-- executes an always-on rule.

UPDATE peg_rules
   SET threshold_price = 1.0,
       enabled = FALSE,
       paused_at = COALESCE(paused_at, NOW())
 WHERE threshold_price > 1.0;

UPDATE peg_rules
   SET target_asset = NULL
 WHERE target_asset IS NOT NULL
   AND UPPER(target_asset) = UPPER(asset);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'peg_rules_threshold_price_depeg_check'
  ) THEN
    ALTER TABLE peg_rules
      ADD CONSTRAINT peg_rules_threshold_price_depeg_check
      CHECK (threshold_price > 0 AND threshold_price <= 1.0);
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'peg_rules_asset_allowed_check'
  ) THEN
    ALTER TABLE peg_rules
      ADD CONSTRAINT peg_rules_asset_allowed_check
      CHECK (UPPER(asset) IN ('USDC', 'EURC', 'USYC'));
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'peg_rules_target_asset_allowed_check'
  ) THEN
    ALTER TABLE peg_rules
      ADD CONSTRAINT peg_rules_target_asset_allowed_check
      CHECK (
        target_asset IS NULL
        OR (
          UPPER(target_asset) IN ('USDC', 'EURC', 'USYC')
          AND UPPER(target_asset) <> UPPER(asset)
        )
      );
  END IF;
END $$;
