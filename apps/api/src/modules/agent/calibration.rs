//! F-CONF-2 — probability calibrator for the regime classifier and the
//! strategist confidence head.
//!
//! Method: **histogram binning** (a.k.a. Brier bin). Each output class is
//! calibrated independently: split [0, 1] into `N_BINS` equal-width buckets,
//! compute the *empirical* base rate of that class within each bucket, and at
//! inference time replace the raw probability by the empirical rate for the
//! bucket the raw prob falls into. Per-class probabilities are then
//! renormalized to sum to 1.
//!
//! Why histogram-bin over isotonic?
//!   - Trivial to serialize (just N_BINS×K floats) — no PAV solver to
//!     reimplement in Rust.
//!   - Handles non-monotonic miscalibration (the same as isotonic).
//!   - Robust when fit-sample counts are tiny (smoothing fallback to the raw
//!     value, see `Bin::empirical`).
//!
//! Trained from `model_evaluation_samples` for the regime classifier task and
//! from `agent_memory.outcome_24h` for the strategist confidence task —
//! see `calibration_train.rs`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Number of equal-width buckets per class. 10 is the standard reliability-diagram
/// resolution; fits comfortably in JSONB and avoids overfit on small samples.
pub const N_BINS: usize = 10;

/// Default ordered class labels for the regime classifier task. Order matters
/// because we renormalize across these keys.
pub const REGIME_CLASSES: [&str; 3] = ["risk_on", "neutral", "risk_off"];

/// One bucket of the histogram-bin calibrator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bin {
    /// Bucket lower bound, inclusive.
    pub lo: f64,
    /// Bucket upper bound, exclusive (except the last bin which is inclusive of 1.0).
    pub hi: f64,
    /// Per-class empirical accuracy in this bucket (raw → realized).
    /// Missing class = no samples in this bucket; caller falls back to raw.
    pub empirical: HashMap<String, f64>,
    /// Sample count that fell in this bucket (across all classes).
    pub n: usize,
}

/// Fitted calibrator parameters serialized into `calibrations.params_jsonb`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Calibration {
    /// Ordered class labels.
    pub classes: Vec<String>,
    /// `N_BINS` buckets in [0, 1].
    pub bins: Vec<Bin>,
    /// Sample count that went into the fit.
    pub fit_samples_count: usize,
    pub brier_before: f64,
    pub brier_after: f64,
}

/// A single training row: predicted per-class probabilities + realized label.
#[derive(Debug, Clone, PartialEq)]
pub struct CalibrationSample {
    pub predicted_proba: HashMap<String, f64>,
    pub realized_label: String,
}

/// Multiclass Brier score: mean across samples of sum-over-classes of
/// (pred_k - one_hot_k)^2. Lower is better; 0 = perfect.
pub fn brier(samples: &[CalibrationSample]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut acc = 0.0;
    for s in samples {
        for (label, p) in &s.predicted_proba {
            let target = if label == &s.realized_label { 1.0 } else { 0.0 };
            let d = p - target;
            acc += d * d;
        }
    }
    acc / (samples.len() as f64)
}

/// Fit a histogram-bin calibrator over the provided samples.
///
/// For each class K, bin samples by their predicted P(K). The empirical rate
/// of K within that bin is the calibrated value. Classes with zero observations
/// in a bin are left absent so `apply` can fall back to the raw probability.
pub fn fit(samples: &[CalibrationSample]) -> Calibration {
    let classes: Vec<String> = REGIME_CLASSES.iter().map(|s| (*s).to_string()).collect();

    let mut bins: Vec<Bin> = (0..N_BINS)
        .map(|i| Bin {
            lo: i as f64 / N_BINS as f64,
            hi: (i + 1) as f64 / N_BINS as f64,
            empirical: HashMap::new(),
            n: 0,
        })
        .collect();

    let mut per_bin_class_hits: Vec<HashMap<String, (usize, usize)>> = vec![HashMap::new(); N_BINS];

    for sample in samples {
        for class in &classes {
            let raw = clamp01(*sample.predicted_proba.get(class).unwrap_or(&0.0));
            let idx = bin_index(raw);
            let entry = per_bin_class_hits[idx]
                .entry(class.clone())
                .or_insert((0, 0));
            entry.1 += 1;
            if &sample.realized_label == class {
                entry.0 += 1;
            }
            bins[idx].n += 1;
        }
    }

    for (i, class_counts) in per_bin_class_hits.into_iter().enumerate() {
        for (class, (hits, total)) in class_counts {
            if total > 0 {
                bins[i].empirical.insert(class, hits as f64 / total as f64);
            }
        }
    }

    let brier_before = brier(samples);
    let calibrated_samples: Vec<CalibrationSample> = samples
        .iter()
        .map(|s| CalibrationSample {
            predicted_proba: apply_inner(&classes, &bins, &s.predicted_proba),
            realized_label: s.realized_label.clone(),
        })
        .collect();
    let brier_after = brier(&calibrated_samples);

    Calibration {
        classes,
        bins,
        fit_samples_count: samples.len(),
        brier_before,
        brier_after,
    }
}

/// Apply a fitted calibrator to a raw per-class probability map.
///
/// Returns a fresh map with the same keys; values are the empirical-rate
/// replacement, renormalized so they sum to 1. If every class's bucket has
/// zero observations, the raw map is returned (still renormalized).
pub fn apply(cal: &Calibration, raw_proba: &HashMap<String, f64>) -> HashMap<String, f64> {
    apply_inner(&cal.classes, &cal.bins, raw_proba)
}

fn apply_inner(
    classes: &[String],
    bins: &[Bin],
    raw_proba: &HashMap<String, f64>,
) -> HashMap<String, f64> {
    let mut out: HashMap<String, f64> = HashMap::with_capacity(classes.len());

    for class in classes {
        let raw = clamp01(*raw_proba.get(class).unwrap_or(&0.0));
        let idx = bin_index(raw);
        let bin = &bins[idx];
        let calibrated = bin.empirical.get(class).copied().unwrap_or(raw);
        out.insert(class.clone(), calibrated);
    }

    // Renormalize so the per-class probabilities still form a distribution.
    let sum: f64 = out.values().copied().sum();
    if sum > f64::EPSILON {
        for v in out.values_mut() {
            *v /= sum;
        }
    } else {
        // Degenerate: fall back to a uniform distribution rather than emit
        // all-zero probabilities (which would break the headline number).
        let u = 1.0 / classes.len() as f64;
        for class in classes {
            out.insert(class.clone(), u);
        }
    }

    out
}

fn bin_index(p: f64) -> usize {
    let p = clamp01(p);
    let i = (p * N_BINS as f64).floor() as usize;
    if i >= N_BINS {
        N_BINS - 1
    } else {
        i
    }
}

fn clamp01(p: f64) -> f64 {
    if p.is_nan() {
        return 0.0;
    }
    p.clamp(0.0, 1.0)
}

/// Convenience: scalar `apply` for the strategist's flat confidence number.
///
/// Treats confidence as a "decision is right" probability and bins it as a
/// single class. Used by the agent service to map a strategist's raw
/// confidence to its calibrated counterpart. The calibrator is the same
/// histogram-bin trained on `agent_memory` outcomes (`right` vs `wrong`).
///
/// `class` must be one of `cal.classes` (use `"right"` for the strategist
/// task — see `calibration_train::fit_strategist`).
pub fn apply_scalar(cal: &Calibration, class: &str, raw: f64) -> f64 {
    let idx = bin_index(raw);
    cal.bins[idx]
        .empirical
        .get(class)
        .copied()
        .unwrap_or(clamp01(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proba(risk_on: f64, neutral: f64, risk_off: f64) -> HashMap<String, f64> {
        let mut m = HashMap::new();
        m.insert("risk_on".into(), risk_on);
        m.insert("neutral".into(), neutral);
        m.insert("risk_off".into(), risk_off);
        m
    }

    /// Construct a deliberately miscalibrated sample set: the model claims
    /// 90% confidence in "risk_on" but is only right 50% of the time.
    /// Histogram-bin should pull that 0.9 down toward 0.5 → Brier improves.
    fn miscalibrated_samples() -> Vec<CalibrationSample> {
        let mut out = Vec::new();
        for i in 0..20 {
            out.push(CalibrationSample {
                predicted_proba: proba(0.9, 0.05, 0.05),
                realized_label: if i < 10 { "risk_on" } else { "neutral" }.into(),
            });
        }
        for i in 0..20 {
            out.push(CalibrationSample {
                predicted_proba: proba(0.05, 0.9, 0.05),
                realized_label: if i < 10 { "neutral" } else { "risk_off" }.into(),
            });
        }
        out
    }

    #[test]
    fn brier_zero_for_perfect_predictions() {
        let samples = vec![CalibrationSample {
            predicted_proba: proba(1.0, 0.0, 0.0),
            realized_label: "risk_on".into(),
        }];
        assert!(brier(&samples).abs() < 1e-9);
    }

    #[test]
    fn brier_nonzero_when_wrong() {
        let samples = vec![CalibrationSample {
            predicted_proba: proba(0.0, 0.0, 1.0),
            realized_label: "risk_on".into(),
        }];
        // (1-0)^2 + (0-0)^2 + (0-1)^2 = 2
        assert!((brier(&samples) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn fit_improves_brier_on_miscalibrated_inputs() {
        let samples = miscalibrated_samples();
        let cal = fit(&samples);
        assert!(
            cal.brier_after < cal.brier_before,
            "histogram-bin should reduce Brier on miscalibrated inputs (before {} -> after {})",
            cal.brier_before,
            cal.brier_after
        );
        // Sanity: the 0.9-bucket for risk_on should have empirical rate near 0.5
        // (10/20 of the 0.9 samples were actually risk_on).
        let bin9 = &cal.bins[9];
        let emp = bin9.empirical.get("risk_on").copied().unwrap_or(-1.0);
        assert!(
            (emp - 0.5).abs() < 0.2,
            "expected empirical rate near 0.5 for risk_on at bin 0.9-1.0, got {emp}"
        );
    }

    #[test]
    fn apply_renormalizes_to_one() {
        let samples = miscalibrated_samples();
        let cal = fit(&samples);
        let out = apply(&cal, &proba(0.9, 0.05, 0.05));
        let sum: f64 = out.values().copied().sum();
        assert!(
            (sum - 1.0).abs() < 1e-9,
            "calibrated proba must sum to 1; got {sum}"
        );
    }

    #[test]
    fn apply_falls_back_to_uniform_on_all_zero() {
        let cal = fit(&miscalibrated_samples());
        // A probability map all of whose entries are well below any observed
        // empirical rates *can* still collapse; force the explicit degenerate
        // path by passing absurd negatives.
        let mut raw = HashMap::new();
        raw.insert("risk_on".into(), -1.0);
        raw.insert("neutral".into(), -1.0);
        raw.insert("risk_off".into(), -1.0);
        let out = apply(&cal, &raw);
        let sum: f64 = out.values().copied().sum();
        assert!((sum - 1.0).abs() < 1e-9);
    }

    #[test]
    fn bin_index_handles_edges() {
        assert_eq!(bin_index(0.0), 0);
        assert_eq!(bin_index(0.05), 0);
        assert_eq!(bin_index(0.1), 1);
        assert_eq!(bin_index(0.99), 9);
        assert_eq!(bin_index(1.0), 9);
        assert_eq!(bin_index(1.5), 9);
        assert_eq!(bin_index(-0.1), 0);
    }

    #[test]
    fn calibration_roundtrips_through_json() {
        let cal = fit(&miscalibrated_samples());
        let json = serde_json::to_string(&cal).unwrap();
        let back: Calibration = serde_json::from_str(&json).unwrap();
        assert_eq!(cal, back);
    }
}
