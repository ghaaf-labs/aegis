//! F-CONF-8 integration test — drives `calibration_train::tick` against a
//! real Postgres so we exercise the full SQL contract:
//!   1. Insert a fake `model_evaluations` + `model_evaluation_samples` set.
//!   2. Run one trainer tick.
//!   3. Assert exactly one `calibrations` row landed for `regime_classifier`
//!      with the right model slug and a Brier improvement.
//!
//! Skipped when `TEST_DATABASE_URL` is unset so CI stays hermetic. Run
//! locally:
//!
//!     docker compose up -d postgres
//!     createdb -h localhost -U aegis aegis_test || true
//!     export TEST_DATABASE_URL=postgres://aegis:aegis@localhost:5432/aegis_test
//!     cargo test --test calibration_train_http -- --nocapture

use aegis_api::modules::agent::calibration_train;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use uuid::Uuid;

#[tokio::test]
async fn trainer_persists_calibrations_row_for_regime() {
    let Ok(db_url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("TEST_DATABASE_URL not set; skipping calibration_train_http integration test");
        return;
    };

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&db_url)
        .await
        .expect("connect to TEST_DATABASE_URL");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");

    // Seed a fake backtest run with deliberately miscalibrated samples so
    // we can assert Brier improves.
    let eval_run_id = Uuid::new_v4();
    let model_slug = format!("test-model/{eval_run_id}");

    sqlx::query(
        r#"
        INSERT INTO model_evaluations
            (id, model_slug, eval_run_id, task, period_start, period_end,
             samples_count, accuracy, precision_macro, recall_macro, f1_macro,
             brier_score, confusion_jsonb, per_regime_jsonb)
        VALUES
            (gen_random_uuid(), $1, $2, 'regime_classifier',
             '2025-01-01'::date, '2026-01-01'::date,
             40, 0.5, 0.5, 0.5, 0.5, 0.4,
             '{"rows":[[0,0,0],[0,0,0],[0,0,0]]}'::jsonb,
             '{"risk_on":{"precision":0.5,"recall":0.5,"f1":0.5,"support":20}}'::jsonb)
        "#,
    )
    .bind(&model_slug)
    .bind(eval_run_id)
    .execute(&pool)
    .await
    .expect("insert model_evaluations row");

    // 20 samples claiming 90% risk_on, half actually realized risk_on /
    // half actually neutral. Histogram-bin should pull that 0.9 toward 0.5.
    for i in 0..20 {
        let realized = if i < 10 { "risk_on" } else { "neutral" };
        sqlx::query(
            r#"
            INSERT INTO model_evaluation_samples
                (eval_run_id, observed_at, predicted_label, predicted_proba,
                 realized_label, features_jsonb)
            VALUES ($1, NOW(), 'risk_on', $2::jsonb, $3, '{}'::jsonb)
            "#,
        )
        .bind(eval_run_id)
        .bind(json!({"risk_on": 0.9, "neutral": 0.05, "risk_off": 0.05}).to_string())
        .bind(realized)
        .execute(&pool)
        .await
        .expect("insert sample");
    }
    // 20 more samples claiming 90% neutral, half realized neutral, half
    // realized risk_off.
    for i in 0..20 {
        let realized = if i < 10 { "neutral" } else { "risk_off" };
        sqlx::query(
            r#"
            INSERT INTO model_evaluation_samples
                (eval_run_id, observed_at, predicted_label, predicted_proba,
                 realized_label, features_jsonb)
            VALUES ($1, NOW(), 'neutral', $2::jsonb, $3, '{}'::jsonb)
            "#,
        )
        .bind(eval_run_id)
        .bind(json!({"risk_on": 0.05, "neutral": 0.9, "risk_off": 0.05}).to_string())
        .bind(realized)
        .execute(&pool)
        .await
        .expect("insert sample");
    }

    let before_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM calibrations WHERE source_eval_run_id = $1")
            .bind(eval_run_id)
            .fetch_one(&pool)
            .await
            .expect("count before");
    assert_eq!(before_count, 0);

    // Drive one trainer pass.
    let id = calibration_train::fit_regime(&pool)
        .await
        .expect("fit_regime ok")
        .expect("fit_regime returned an id");

    let row = sqlx::query(
        r#"
        SELECT model_slug, method, fit_samples_count, brier_before, brier_after
        FROM calibrations
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .expect("select calibrations row");

    let persisted_slug: String = row.get("model_slug");
    let method: String = row.get("method");
    let n: i32 = row.get("fit_samples_count");
    let before: f64 = row.get("brier_before");
    let after: f64 = row.get("brier_after");

    assert_eq!(persisted_slug, model_slug);
    assert_eq!(method, "brier_bin");
    assert_eq!(n, 40);
    assert!(
        after < before,
        "trainer must produce a calibration with Brier improvement ({before} -> {after})"
    );

    // Cleanup so reruns against a long-lived DB stay tidy.
    let _ = sqlx::query("DELETE FROM calibrations WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM model_evaluation_samples WHERE eval_run_id = $1")
        .bind(eval_run_id)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM model_evaluations WHERE eval_run_id = $1")
        .bind(eval_run_id)
        .execute(&pool)
        .await;
}
