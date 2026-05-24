/**
 * Aegis Tailwind config — neo-brutalism dark, dual-accent.
 *
 * Tokens documented in `docs/04-design-system.md`. The strict rule is:
 *   - `accent-pnl` (green) for money / PnL / approvals only.
 *   - `accent-agent` (cyan) for agent activity / model output / regime signals only.
 *   - Never mix in the same component.
 */
/** @type {import('tailwindcss').Config} */
const config = {
  darkMode: ["class"],
  content: ["./src/**/*.{ts,tsx}", "../../packages/ui/src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // Neo-brutalism surfaces.
        bg: "#0A0A0A",
        surface: "#141414",
        raised: "#1C1C1C",

        // Borders.
        "border-default": "#2A2A2A",
        "border-hi": "#FFFFFF",

        // Two-accent semantic system.
        "accent-pnl": "#00FF88", // green — money / approvals
        "accent-agent": "#00E0FF", // cyan — agent activity
        risk: "#FF2D7A",
        warn: "#FFB800",

        // Text scale.
        "text-hi": "#FFFFFF",
        "text-default": "#E5E5E5",
        // #969696 → 5.76:1 on raised (#1C1C1C), 6.20:1 on bg — clears WCAG AA on all surfaces.
        // Prior #8A8A8A was only 4.94:1 on raised, failing AA for small labels on cards.
        "text-lo": "#969696",
        // #909090 → 5.34:1 on raised (#1C1C1C), 6.20:1 on bg — clears WCAG AA on all surfaces.
        // Prior #7C7C7C was 4.08:1 on raised and 4.41:1 on surface, both below AA threshold.
        "text-mut": "#909090",

        // Shadcn legacy tokens (kept while components migrate).
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
      },
      // Hard offset shadows — never blur.
      boxShadow: {
        brutal: "4px 4px 0 0 #000000",
        "brutal-sm": "2px 2px 0 0 #000000",
        "brutal-lg": "6px 6px 0 0 #000000",
      },
      borderRadius: {
        // Small radius only. No rounded-full, no rounded-2xl.
        sharp: "2px",
        card: "4px",
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
      borderWidth: {
        brutal: "2px",
      },
      fontFamily: {
        // Replaced in apps/web/src/app/layout.tsx with next/font once the
        // design sweep ships; safe defaults here for primitives previewed
        // outside Next.js.
        sans: [
          "Inter Tight",
          "var(--font-inter-tight)",
          "system-ui",
          "sans-serif",
        ],
        mono: [
          "JetBrains Mono",
          "var(--font-jetbrains-mono)",
          "ui-monospace",
          "monospace",
        ],
      },
      fontVariantNumeric: {
        tabular: "tabular-nums",
      },
      keyframes: {
        "accordion-down": {
          from: { height: "0" },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: "0" },
        },
        shimmer: {
          "100%": { transform: "translateX(100%)" },
        },
        pulse_glow: {
          "0%, 100%": { opacity: "1" },
          "50%": { opacity: "0.5" },
        },
      },
      animation: {
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
        shimmer: "shimmer 2s infinite",
        pulse_glow: "pulse_glow 2s ease-in-out infinite",
      },
    },
  },
  plugins: [require("tailwindcss-animate")],
};

module.exports = config;
