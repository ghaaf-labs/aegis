//! F-CONF-5 — counterfactual second-pass for the critic.
//!
//! After the existing adversarial verdict (handled inline in `service.rs`),
//! this module fires *one additional* critic call asking the model to
//! produce a one-sentence counterfactual answering: "If the regime had been
//! classified differently, would this proposal still fire?".
//!
//! Output is a small JSON object `{ "verdict": "...", "counterfactual": "..." }`;
//! we keep `verdict` so the same prompt template can be reused for ad-hoc
//! human-readable summaries and to ensure the model emits *something*
//! structured even when the counterfactual itself is "n/a".
//!
//! Gated by `CALIBRATED_CONF_ENABLED` in the caller — this module just
//! exposes the prompt builder and the parser.

use serde::{Deserialize, Serialize};

/// Strict shape we accept from the second-pass critic. Both fields default
/// to empty so a malformed model response degrades to "no counterfactual"
/// rather than failing the whole decision.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CounterfactualOutput {
    #[serde(default)]
    pub verdict: String,
    #[serde(default)]
    pub counterfactual: String,
}

impl CounterfactualOutput {
    pub fn is_empty(&self) -> bool {
        self.counterfactual.trim().is_empty()
    }
}

/// Render the second-pass critic prompt. Inputs are the raw strings the
/// agent service already has on hand: the proposal JSON, the regime label,
/// and the original critic verdict text. We avoid re-rendering the full
/// strategist context to keep the second call cheap (latency budget ≈ 5s).
pub fn build_prompt(proposal_json: &str, regime: &str, original_verdict_notes: &str) -> String {
    format!(
        r#"You are the critic. Below is a portfolio rebalance proposal a strategist just emitted, the regime it was conditioned on, and your first-pass verdict notes. Your job in this second pass is to produce a single, concrete counterfactual that helps the user decide whether to trust the proposal.

Imagine ONE single, named change to the inputs: either the regime classification was different (risk_on ↔ neutral ↔ risk_off) OR a single named market feature flipped sign (e.g. btcVol30d, corr90d, fearGreed). State whether the proposal would still fire under that counterfactual world.

Output STRICT JSON only, no prose, no markdown fences:
{{
  "verdict": "approved" | "revised" | "flagged",
  "counterfactual": "If <named single feature> had <opposite sign / different regime>, this rebalance would <still fire | NOT fire | partially fire>."
}}

The counterfactual MUST be one sentence, start with "If ", and name a single concrete input.

PROPOSAL JSON:
{proposal_json}

REGIME: {regime}

FIRST-PASS VERDICT NOTES:
{original_verdict_notes}
"#
    )
}

/// Strict JSON parser. Strips the same OpenAI/DeepSeek markdown fences the
/// rest of the agent loop tolerates, then deserializes. Returns an `Err`
/// only on truly malformed JSON — the empty-string default ensures partial
/// outputs still surface to the user.
pub fn parse(raw: &str) -> anyhow::Result<CounterfactualOutput> {
    let stripped = crate::modules::ai::strip_json_fences(raw);
    serde_json::from_str(stripped)
        .map_err(|e| anyhow::anyhow!("invalid counterfactual JSON: {e}; raw: {raw}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_camel_case_json() {
        let raw = r#"{"verdict":"approved","counterfactual":"If regime had stayed RISK_ON, this rebalance would not fire."}"#;
        let out = parse(raw).unwrap();
        assert_eq!(out.verdict, "approved");
        assert!(out.counterfactual.contains("RISK_ON"));
    }

    #[test]
    fn parses_through_markdown_fence() {
        let raw = "```json\n{\"verdict\":\"flagged\",\"counterfactual\":\"If btcVol30d had been below 0.4, this rebalance would still fire.\"}\n```";
        let out = parse(raw).unwrap();
        assert_eq!(out.verdict, "flagged");
        assert!(out.counterfactual.starts_with("If btcVol30d"));
    }

    #[test]
    fn empty_fields_default_to_empty_string() {
        let out: CounterfactualOutput = serde_json::from_str("{}").unwrap();
        assert!(out.is_empty());
        assert_eq!(out.verdict, "");
    }

    #[test]
    fn invalid_json_returns_err() {
        assert!(parse("not json at all").is_err());
    }

    #[test]
    fn build_prompt_includes_proposal_and_regime() {
        let p = build_prompt("{\"summary\":\"Hold\"}", "risk_off", "looks fine");
        assert!(p.contains("Hold"));
        assert!(p.contains("risk_off"));
        assert!(p.contains("counterfactual"));
        // Locks the contract: the second-pass prompt must demand strict JSON.
        assert!(p.contains("STRICT JSON"));
    }
}
