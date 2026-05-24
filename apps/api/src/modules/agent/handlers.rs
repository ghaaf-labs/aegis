use axum::{
    extract::{Path, State},
    Extension, Json,
};
use uuid::Uuid;

use super::{
    models::{AgentDecision, AnalyzeRequest, ProposeAllocationRequest},
    service,
};
use crate::modules::portfolio::models::Portfolio;
use crate::modules::rebalance::registry::{
    capabilities::RuntimeCapabilities, route::route_state_for_token,
};
use crate::modules::scheduler::tick::adopt_and_execute_autopilot_proposal;
use crate::{config::Config, middleware::auth::Claims, router::AppState};

/// Annotate a decision with the live per-symbol execution-readiness of its
/// proposed `recommended_allocation`, so the approval modal can badge each
/// sleeve truthfully ("Executes now" vs "Track only") from the route engine
/// rather than a static client guess. No-op for decisions without an allocation.
fn attach_route_states(decision: &mut AgentDecision, config: &Config) {
    let Some(symbols) = decision
        .recommended_allocation
        .as_ref()
        .and_then(serde_json::Value::as_object)
    else {
        return;
    };
    let caps = RuntimeCapabilities::from_config(config);
    let states: serde_json::Map<String, serde_json::Value> = symbols
        .keys()
        .map(|symbol| {
            let state = route_state_for_token(&caps, config, symbol);
            (symbol.clone(), serde_json::json!(state))
        })
        .collect();
    decision.route_states = Some(serde_json::Value::Object(states));
}

pub async fn decisions(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(portfolio_id): Path<Uuid>,
) -> crate::error::Result<Json<Vec<AgentDecision>>> {
    // Only terminal `ready` rows reach the client: a `queued`/`running`
    // placeholder must not surface as a pending proposal (it would open Gate-1
    // on an empty decision), and `failed` rows are recovered via the per-id poll.
    let decisions = sqlx::query_as::<_, AgentDecision>(
        "SELECT d.* FROM agent_decisions d
         JOIN portfolios p ON p.id = d.portfolio_id
         WHERE d.portfolio_id = $1 AND p.user_id = $2 AND d.status = 'ready'
         ORDER BY d.created_at DESC LIMIT 50",
    )
    .bind(portfolio_id)
    .bind(claims.sub)
    .fetch_all(&state.db)
    .await?;

    let mut decisions = decisions;
    for decision in &mut decisions {
        attach_route_states(decision, &state.config);
    }
    Ok(Json(decisions))
}

/// Fetch a single decision by id, scoped to the caller's ownership of the
/// underlying portfolio. The approval modal needs the model_slug, critic
/// verdict, and confidence to surface alongside the plan — Agentic
/// Sophistication is 30% of the judging weight and judges grade on whether
/// the agent's reasoning is visible at the moment of approval.
pub async fn decision_by_id(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(decision_id): Path<Uuid>,
) -> crate::error::Result<Json<AgentDecision>> {
    let decision = sqlx::query_as::<_, AgentDecision>(
        "SELECT d.* FROM agent_decisions d
         JOIN portfolios p ON p.id = d.portfolio_id
         WHERE d.id = $1 AND p.user_id = $2",
    )
    .bind(decision_id)
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound(format!("decision {decision_id}")))?;
    let mut decision = decision;
    attach_route_states(&mut decision, &state.config);
    Ok(Json(decision))
}

pub async fn analyze(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<AnalyzeRequest>,
) -> crate::error::Result<Json<AgentDecision>> {
    // Verify the caller owns the portfolio before running the AI pipeline.
    let owned: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM portfolios WHERE id = $1 AND user_id = $2")
            .bind(body.portfolio_id)
            .bind(claims.sub)
            .fetch_optional(&state.db)
            .await?;

    if owned.is_none() {
        return Err(crate::error::AppError::NotFound(format!(
            "portfolio {}",
            body.portfolio_id
        )));
    }

    // Async: enqueue a `queued` decision and return it immediately (the
    // strategist→critic→revision pipeline can exceed nginx's 60s cap). The
    // spawned, semaphore-bounded job flips the row to `ready`/`failed`; the
    // client polls the decision id and opens on the SSE `agent.decision` event.
    let (queued, is_new) = service::enqueue_analysis(&state, &body).await?;
    if is_new {
        let st = state.clone();
        let decision_id = queued.id;
        tokio::spawn(async move {
            if let Err(e) = service::run_analysis_job(&st, decision_id, &body).await {
                tracing::warn!(error = %e, decision_id = %decision_id, "spawned analysis job failed");
            }
        });
    }
    Ok(Json(queued))
}

/// Run the allocator — the agent designs a target allocation for the user's
/// portfolio (Gate-1 proposal). Ownership-checked before the AI pipeline runs.
pub async fn propose_allocation(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<ProposeAllocationRequest>,
) -> crate::error::Result<Json<AgentDecision>> {
    let Some((_, auto_pilot)) = sqlx::query_as::<_, (Uuid, bool)>(
        "SELECT p.id, u.auto_pilot_enabled
         FROM portfolios p
         JOIN users u ON u.id = p.user_id
         WHERE p.id = $1 AND p.user_id = $2",
    )
    .bind(body.portfolio_id)
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?
    else {
        return Err(crate::error::AppError::NotFound(format!(
            "portfolio {}",
            body.portfolio_id
        )));
    };

    // Async: enqueue a `queued` decision and return it immediately so the request
    // never blocks on the 38–240s pipeline (nginx caps the API at 60s). The slow
    // work runs in a spawned, semaphore-bounded job that flips the row to
    // `ready`/`failed`; the client opens Gate-1 on the SSE `agent.decision` event
    // or by polling the decision id. When auto-pilot is on, the same background
    // task adopts and executes the ready proposal through the autonomous guardrail
    // path, so the user-triggered onboarding flow matches scheduler auto-pilot.
    let (queued, is_new) = service::enqueue_allocation(&state, &body).await?;
    let user_id = claims.sub;
    if is_new || auto_pilot {
        let st = state.clone();
        let decision_id = queued.id;
        tokio::spawn(async move {
            let outcome = if is_new {
                service::run_allocation_job(&st, decision_id, &body).await
            } else {
                service::await_decision_ready(&st, decision_id).await
            };
            match outcome {
                Ok(decision) if auto_pilot => {
                    if let Err(e) =
                        adopt_and_execute_autopilot_proposal(&st, decision, user_id).await
                    {
                        tracing::warn!(error = %e, decision_id = %decision_id, "auto-pilot adoption after allocation failed");
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, decision_id = %decision_id, "spawned allocation job failed");
                }
            }
        });
    }
    Ok(Json(queued))
}

/// Approve an allocation proposal — write the agent's target into the
/// portfolio (Gate-1 approval). Ownership is enforced inside the service.
pub async fn approve_allocation(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(decision_id): Path<Uuid>,
) -> crate::error::Result<Json<Portfolio>> {
    let portfolio = service::apply_allocation(&state, decision_id, claims.sub).await?;
    Ok(Json(portfolio))
}
