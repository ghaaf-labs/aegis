//! Prompt registry — loads `apps/api/prompts/*.md` at boot, renders templates
//! with `{{ placeholder }}` substitution.
//!
//! Every prompt ships as on-disk markdown so iteration doesn't require a
//! recompile. Compile-time `include_str!` fallbacks guarantee the binary
//! always has a usable prompt even if the deploy environment doesn't ship
//! the `prompts/` directory next to the binary.

use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptKey {
    Strategist,
    Critic,
    Regime,
    Revision,
    Tax,
    Commentary,
}

impl PromptKey {
    /// Filename relative to `apps/api/prompts/`.
    pub fn filename(self) -> &'static str {
        match self {
            Self::Strategist => "strategist.md",
            Self::Critic => "critic.md",
            Self::Regime => "regime.md",
            Self::Revision => "revision.md",
            Self::Tax => "tax.md",
            Self::Commentary => "commentary.md",
        }
    }

    /// Compile-time embedded template — used as fallback when the on-disk
    /// file is missing, and as a guard against drift between source and prod.
    pub fn embedded(self) -> &'static str {
        match self {
            Self::Strategist => include_str!("../../../prompts/strategist.md"),
            Self::Critic => include_str!("../../../prompts/critic.md"),
            Self::Regime => include_str!("../../../prompts/regime.md"),
            Self::Revision => include_str!("../../../prompts/revision.md"),
            Self::Tax => include_str!("../../../prompts/tax.md"),
            Self::Commentary => include_str!("../../../prompts/commentary.md"),
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Strategist,
            Self::Critic,
            Self::Regime,
            Self::Revision,
            Self::Tax,
            Self::Commentary,
        ]
    }
}

pub struct PromptRegistry {
    templates: HashMap<PromptKey, String>,
}

impl PromptRegistry {
    /// Load every prompt. If `PROMPTS_DIR` is set, prefer that directory;
    /// fall back to compiled-in templates on any I/O error.
    pub async fn load() -> Self {
        let dir = std::env::var("PROMPTS_DIR").ok().map(PathBuf::from);
        let mut templates = HashMap::with_capacity(PromptKey::all().len());

        for &key in PromptKey::all() {
            let loaded = match dir.as_ref() {
                Some(d) => tokio::fs::read_to_string(d.join(key.filename())).await.ok(),
                None => None,
            };
            templates.insert(key, loaded.unwrap_or_else(|| key.embedded().to_string()));
        }

        Self { templates }
    }

    /// Build from the embedded fallbacks. Useful in tests and as a deploy
    /// safety net when no filesystem prompts directory is configured.
    #[allow(dead_code)]
    pub fn embedded() -> Self {
        let mut templates = HashMap::with_capacity(PromptKey::all().len());
        for &key in PromptKey::all() {
            templates.insert(key, key.embedded().to_string());
        }
        Self { templates }
    }

    pub fn get(&self, key: PromptKey) -> &str {
        self.templates
            .get(&key)
            .map(String::as_str)
            .unwrap_or_else(|| key.embedded())
    }

    /// Render a template, substituting `{{ name }}` placeholders. Whitespace
    /// inside the braces is tolerated: `{{name}}`, `{{ name }}`, and
    /// `{{  name  }}` all match.
    ///
    /// Missing keys render as the placeholder unchanged so tests can spot
    /// them; this never panics.
    pub fn render(&self, key: PromptKey, ctx: &HashMap<&str, String>) -> String {
        let template = self.get(key);
        render_template(template, ctx)
    }
}

/// Free function for use in tests and benchmarks.
pub fn render_template(template: &str, ctx: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(end) = find_close(bytes, i + 2) {
                let name = std::str::from_utf8(&bytes[i + 2..end]).unwrap_or("").trim();
                match ctx.get(name) {
                    Some(value) => out.push_str(value),
                    None => out.push_str(&template[i..end + 2]),
                }
                i = end + 2;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn find_close(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 1 < bytes.len() {
        if bytes[i] == b'}' && bytes[i + 1] == b'}' {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<const N: usize>(pairs: [(&'static str, &str); N]) -> HashMap<&'static str, String> {
        pairs.into_iter().map(|(k, v)| (k, v.to_string())).collect()
    }

    #[test]
    fn embedded_templates_are_non_empty() {
        for &k in PromptKey::all() {
            assert!(!k.embedded().is_empty(), "embedded template missing: {k:?}");
        }
    }

    #[test]
    fn render_substitutes_placeholders() {
        let template = "Hello {{ name }}, you are {{role}}.";
        let result = render_template(template, &ctx([("name", "Alice"), ("role", "engineer")]));
        assert_eq!(result, "Hello Alice, you are engineer.");
    }

    #[test]
    fn render_tolerates_inner_whitespace() {
        let template = "{{x}} {{  y  }}";
        let result = render_template(template, &ctx([("x", "1"), ("y", "2")]));
        assert_eq!(result, "1 2");
    }

    #[test]
    fn render_leaves_unknown_keys_as_placeholders() {
        let template = "Hi {{ missing }}";
        let result = render_template(template, &ctx([]));
        assert_eq!(result, "Hi {{ missing }}");
    }

    #[test]
    fn render_handles_template_without_placeholders() {
        let template = "no placeholders here";
        let result = render_template(template, &ctx([]));
        assert_eq!(result, "no placeholders here");
    }

    #[test]
    fn embedded_registry_renders_strategist() {
        let reg = PromptRegistry::embedded();
        let rendered = reg.render(
            PromptKey::Strategist,
            &ctx([
                ("portfolio_name", "Treasury"),
                ("total_value_usd", "10000.00"),
                ("risk_tolerance", "moderate"),
                ("horizon_months", "12"),
                ("pnl_usd", "250.00"),
                ("pnl_pct", "2.5"),
                ("allocations_table", "BTC 50%"),
                ("regime", "neutral"),
                ("regime_confidence", "0.7"),
                ("btc_vol_30d", "0.45"),
                ("corr_90d", "0.6"),
                ("max_drawdown", "0.15"),
                ("fear_greed", "48"),
                ("btc_dominance", "52"),
                ("concentration_risk", "0.4"),
                ("volatility_score", "0.5"),
                ("drift_score", "0.1"),
                // Sprint 2 placeholders:
                ("goal_block", "(no goal set yet)"),
                ("memory", "- no prior decisions"),
                ("usyc_rate", "0.0510"),
                ("usdc_eurc_basis", "0.9217"),
                // Sprint 3 placeholder:
                ("harvestable_losses", "(none)"),
                // Sprint 4 placeholder — wallet block tells the agent
                // about Gateway balance so it doesn't say "deposit funds"
                // when the user is already funded.
                ("wallet_block", "Wallet balance: \\$0"),
            ]),
        );
        // No unresolved placeholders should remain.
        assert!(
            !rendered.contains("{{"),
            "strategist template has unresolved placeholders: {rendered}"
        );
        assert!(rendered.contains("Treasury"));
        assert!(rendered.contains("moderate"));
    }
}
