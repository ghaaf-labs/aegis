import { dirname } from "path";
import { fileURLToPath } from "url";
import { FlatCompat } from "@eslint/eslintrc";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const compat = new FlatCompat({
  baseDirectory: __dirname,
});

const eslintConfig = [
  ...compat.extends("next/core-web-vitals", "next/typescript"),

  // Code-quality metrics. All `warn` (not `error`) on purpose: `next lint` and
  // `next build` fail only on errors, so these surface complexity/size/duplication
  // without blocking CI. Built-in rules only — no plugin dependency, so the
  // `--frozen-lockfile` install in CI is unaffected. See docs/15-quality-bar.md.
  {
    files: ["src/**/*.{ts,tsx}"],
    rules: {
      complexity: ["warn", { max: 12 }], // cyclomatic complexity per function
      "max-depth": ["warn", { max: 4 }], // nested block depth
      "max-params": ["warn", { max: 4 }], // pass an object past this
      "max-nested-callbacks": ["warn", { max: 3 }],
      "max-statements": ["warn", { max: 25 }],
      "max-lines-per-function": [
        "warn",
        { max: 80, skipBlankLines: true, skipComments: true },
      ],
      "max-lines": [
        "warn",
        { max: 400, skipBlankLines: true, skipComments: true },
      ],
      "no-duplicate-imports": "warn",
      eqeqeq: ["warn", "smart"],
    },
  },

  // Tests and generated/config files legitimately break the size/complexity
  // budgets (long describe blocks, fixtures) — don't nag about those.
  {
    files: [
      "**/*.{test,spec}.{ts,tsx}",
      "e2e/**",
      "**/*.config.{ts,mts,js,mjs}",
    ],
    rules: {
      "max-lines-per-function": "off",
      "max-lines": "off",
      "max-statements": "off",
      complexity: "off",
    },
  },
];

export default eslintConfig;
