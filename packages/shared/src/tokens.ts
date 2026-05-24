// ── Canonical token table (frontend/shared contract) ──────────────────────
//
// The ONE token list the frontend derives from: friendly labels, the
// designable menu, coming-soon gating, and `AssetSymbol`. Its data lives in
// `tokens.generated.json`, which is the FE projection of the backend registry
// (`apps/api/src/domain/token.rs::TOKEN_REGISTRY`). The Rust test
// `fe_token_contract_matches_generated_json` regenerates + guards that JSON, so
// this table can never silently drift from the backend.

import generated from "./tokens.generated.json";

export type TokenClass = "stable" | "yield" | "fx_stable" | "volatile";

export interface TokenMeta {
  /** Canonical symbol (the API/DB contract value), e.g. "cbBTC". */
  symbol: string;
  /** Friendly display name, e.g. "Bitcoin". */
  label: string;
  /** Whether the AI allocator may assign this sleeve a target weight. */
  designable: boolean;
  /** Designable but gated (USYC) — surface as coming-soon, never investable. */
  comingSoon: boolean;
  class: TokenClass;
}

/** Every token Aegis prices, tracks, or settles — the FE projection of the
 *  backend registry. */
/** Every token Aegis prices, tracks, or settles — the FE projection of the
 *  backend registry. The frontend derives its labels, designable menu, and
 *  coming-soon gating from this one table. */
export const TOKENS: readonly TokenMeta[] = generated as readonly TokenMeta[];
