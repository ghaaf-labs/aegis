# 10 — Observability

A deliberately small surface: structured logs + a four-counter `/metrics` endpoint. Enough for an operator to reconstruct any incident in under a minute, not enough to need a Prometheus + Grafana stack we don't yet have users to fill.

## What's instrumented

| Where                                                     | Signal                                                                               |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `agent::service::record_decision`                         | `aegis_agent_decisions_total` counter +1 on every persisted decision row.            |
| `rebalance::executor::walk_legs` (success path)           | `aegis_rebalances_succeeded_total` +1, `aegis_usdc_moved_cents_total` += plan total. |
| `rebalance::executor::approve_and_execute` (failure path) | `aegis_rebalances_failed_total` +1.                                                  |
| `tracing::info!/warn!/error!` everywhere                  | Structured fields: `rebalance_id`, `decision_id`, `model_slug`, `error`.             |

## `GET /metrics`

Public, no auth. Prometheus text exposition format. Counters only — no histograms or labels.

```
$ curl -s http://localhost:8080/metrics
# HELP aegis_agent_decisions_total Agent decisions persisted.
# TYPE aegis_agent_decisions_total counter
aegis_agent_decisions_total 0
# HELP aegis_rebalances_succeeded_total Rebalances reaching status=completed.
# TYPE aegis_rebalances_succeeded_total counter
aegis_rebalances_succeeded_total 0
# HELP aegis_rebalances_failed_total Rebalances reaching status=failed.
# TYPE aegis_rebalances_failed_total counter
aegis_rebalances_failed_total 0
# HELP aegis_usdc_moved_cents_total Sum of USDC moved by completed rebalances, in cents.
# TYPE aegis_usdc_moved_cents_total counter
aegis_usdc_moved_cents_total 0
```

Counters are process-local (`AtomicU64`) — they reset on restart, by design for this scope. If/when there's a second process or a user counts on long-term durability, swap the storage for the `prometheus` crate's registry.

## Grep recipes

Logs are JSON when the binary runs under a structured subscriber and pretty when running locally. Either way the spans/fields are identical — these recipes work against both.

### "What happened to this rebalance?"

```bash
tail -F /tmp/aegis-api.log | rg "rebalance_id=<uuid>"
```

Covers the full lifecycle (plan → approve → walk_legs → leg state changes → completed/failed). If logs are JSON, swap `rg` for `jq 'select(.rebalance_id=="<uuid>")'`.

### "Which model made this decision?"

```bash
psql "$DATABASE_URL" -c "
SELECT id, model_slug, regime, confidence, latency_ms, prompt_tokens
FROM agent_decisions
WHERE id = '<decision_uuid>';"
```

The `model_slug` column is the OpenRouter slug actually used (subject to budget-guard downshifts). Not derivable from the request payload.

### "Show me everything from the last 24h"

```sql
SELECT
  ad.id, ad.created_at, ad.model_slug, ad.regime, ad.confidence,
  r.status AS rebalance_status,
  r.completed_at
FROM agent_decisions ad
LEFT JOIN rebalances r ON r.decision_id = ad.id
WHERE ad.created_at > NOW() - INTERVAL '24 hours'
ORDER BY ad.created_at DESC;
```

### "Did the protocol fee settle?"

```sql
SELECT r.id, r.status, b.amount_usdc, b.facilitator_status, b.error_reason
FROM rebalances r
LEFT JOIN billing_events b ON b.rebalance_id = r.id
WHERE r.id = '<rebalance_uuid>';
```

## What's NOT here, and why

- **No Prometheus client crate.** A bare `AtomicU64` ships a working `/metrics` without a new dep. The day we want histograms or labeled counters (per chain, per user tier, per model), swap to `prometheus` — schema-compatible.
- **No Grafana dashboards.** Premature at zero users. `curl /metrics` from a one-shot script is enough.
- **No tracing exporter (Jaeger / OTLP).** Local tracing-subscriber prints to stdout; a real exporter lands the day we run multiple processes.
- **No alerting / pages.** No on-call exists. When one does, page on `aegis_rebalances_failed_total` rate-of-change.
- **No structured log file rotation.** Operators run `tail -F` directly today; rotation is a deploy-config concern.

## Adding a metric

1. Define an `AtomicU64` in `apps/api/src/modules/observability/counters.rs`.
2. Add a `record_*()` helper.
3. Add the `# HELP` + `# TYPE` + value lines to `render_prometheus()`.
4. Call the helper at the relevant state transition.

That's the entire surface. Resist labels until you can name a query the counter alone can't answer.
