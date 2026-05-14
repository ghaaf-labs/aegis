# Strategist — revision after critic feedback

The critic found a problem with your previous proposal. Address it.

## Your previous proposal

```json
{{ original_proposal_json }}
```

## Critic's verdict

```json
{{ critic_verdict_json }}
```

## How to revise

You may:

- Adjust trade sizes.
- Drop trades that don't survive the critique.
- Add a trade that addresses the critic's concern (e.g. a hedge).
- Lower your confidence if the critic's evidence is strong.

You should not:

- Change the regime read (that came from the classifier, not you).
- Add trades unrelated to the critic's concern.

## Same portfolio context (reproduced for convenience)

- **Portfolio:** {{ portfolio_name }} — ${{ total_value_usd }}
- **Regime:** {{ regime }}
- **Risk tolerance:** {{ risk_tolerance }}, horizon {{ horizon_months }}mo

{{ allocations_table }}

## Output format

Same shape as the original. Respond with **valid JSON only**:

```json
{
  "reasoning": "2-4 sentences. Acknowledge the critic's point and explain the change.",
  "confidence": 0.0,
  "recommendation": {
    "summary": "Revised one-line headline.",
    "trades": [],
    "expectedImpact": { "riskDelta": 0.0, "diversificationScore": 0.0 }
  }
}
```
