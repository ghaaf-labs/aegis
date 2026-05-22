-- ── 0029 — Account wallet network routes ─────────────────────────────────
--
-- Circle developer-controlled wallets are one account wallet in the product,
-- but Circle stores one wallet row per blockchain. Persist those rows directly
-- so adding future EVM networks is a derive-and-upsert, not another hardcoded
-- column on `users`.

CREATE TABLE IF NOT EXISTS user_wallet_networks (
    user_id          UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    blockchain       TEXT        NOT NULL,
    circle_wallet_id TEXT        NOT NULL,
    address          TEXT        NOT NULL,
    account_type     TEXT        NOT NULL DEFAULT 'SCA',
    wallet_set_id    TEXT,
    state            TEXT        NOT NULL DEFAULT 'LIVE',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (user_id, blockchain)
);

CREATE INDEX IF NOT EXISTS user_wallet_networks_circle_wallet_id_idx
    ON user_wallet_networks(circle_wallet_id);

CREATE INDEX IF NOT EXISTS user_wallet_networks_address_idx
    ON user_wallet_networks(LOWER(address));

DROP TRIGGER IF EXISTS user_wallet_networks_updated_at ON user_wallet_networks;
CREATE TRIGGER user_wallet_networks_updated_at
    BEFORE UPDATE ON user_wallet_networks
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

INSERT INTO user_wallet_networks (
    user_id, blockchain, circle_wallet_id, address, account_type, wallet_set_id, state
)
SELECT id, 'ARC-TESTNET', wallet_id, arc_address, 'SCA', wallet_set_id, 'LIVE'
FROM users
WHERE wallet_id IS NOT NULL
  AND arc_address IS NOT NULL
  AND NOT wallet_id LIKE 'mock_wallet_%'
  AND NOT arc_address LIKE '0xARC%'
ON CONFLICT (user_id, blockchain) DO UPDATE
    SET circle_wallet_id = EXCLUDED.circle_wallet_id,
        address = EXCLUDED.address,
        account_type = EXCLUDED.account_type,
        wallet_set_id = EXCLUDED.wallet_set_id,
        state = EXCLUDED.state;

INSERT INTO user_wallet_networks (
    user_id, blockchain, circle_wallet_id, address, account_type, wallet_set_id, state
)
SELECT id, 'BASE-SEPOLIA', wallet_id, base_address, 'SCA', wallet_set_id, 'LIVE'
FROM users
WHERE wallet_id IS NOT NULL
  AND base_address IS NOT NULL
  AND NOT wallet_id LIKE 'mock_wallet_%'
  AND NOT base_address LIKE '0xBASE%'
ON CONFLICT (user_id, blockchain) DO UPDATE
    SET circle_wallet_id = EXCLUDED.circle_wallet_id,
        address = EXCLUDED.address,
        account_type = EXCLUDED.account_type,
        wallet_set_id = EXCLUDED.wallet_set_id,
        state = EXCLUDED.state;
