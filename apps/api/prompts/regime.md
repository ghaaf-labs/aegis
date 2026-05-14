# Regime classifier — label market regime from precomputed features

You receive **precomputed statistical features**. You do not compute them; you
label the regime they describe.

## Features

```json
{{ features_json }}
```

Where:

- `btc_vol_30d` — BTC 30d realized volatility, annualized.
- `corr_90d` — average pairwise 90d return correlation across the major
  asset basket.
- `max_drawdown` — max peak-to-trough drawdown across the basket over 30d.
- `fear_greed` — current Fear & Greed Index (0–100).
- `btc_dominance` — BTC market cap share (%).

## Decision rules (guidance, not a contract)

- **risk_on** — low/moderate vol, low correlation, no severe drawdown, neutral-
  to-greedy sentiment. Capital deployment makes sense.
- **risk_off** — high vol, correlation spiking toward 1, deep drawdown, or
  fearful sentiment. Defensive posture, increase stable / yield sleeve.
- **neutral** — mixed signals or transitional state.

When multiple regimes look plausible, pick the one with the strongest
evidence and reflect that uncertainty in your `confidence` value.

## Output format

Respond with **valid JSON only**:

```json
{
  "regime": "risk_off",
  "confidence": 0.0,
  "rationale": "1 sentence citing the dominant signal."
}
```

Allowed `regime` values: `risk_on`, `neutral`, `risk_off`.
