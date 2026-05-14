/**
 * Conventional Commits, scoped to this monorepo.
 *
 * Enforces `type(scope): subject` with a fixed list of types and the scopes
 * called out in CLAUDE.md so commit history stays grep-friendly.
 */
module.exports = {
  extends: ["@commitlint/config-conventional"],
  rules: {
    "type-enum": [
      2,
      "always",
      [
        "feat",
        "fix",
        "docs",
        "refactor",
        "chore",
        "ci",
        "test",
        "perf",
        "build",
        "revert",
      ],
    ],
    "scope-enum": [
      1, // warn (not enforce) — scope is optional but encouraged
      "always",
      [
        "web",
        "api",
        "shared",
        "ui",
        "agent",
        "portfolio",
        "auth",
        "wallet",
        "gateway",
        "cctp",
        "yield",
        "fx",
        "tax",
        "sse",
        "ai",
        "risk",
        "docs",
        "contracts",
        "infra",
        "deps",
      ],
    ],
    "subject-case": [2, "never", ["sentence-case", "start-case", "pascal-case"]],
    "subject-empty": [2, "never"],
    "subject-full-stop": [2, "never", "."],
    "header-max-length": [2, "always", 100],
    "body-max-line-length": [1, "always", 100],
  },
};
