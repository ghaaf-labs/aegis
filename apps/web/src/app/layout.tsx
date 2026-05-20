import type { Metadata } from "next";
import { Inter_Tight, JetBrains_Mono } from "next/font/google";
import { Providers } from "@/components/providers";
import "./globals.css";

// Neo-brutalism typography: tight humanist sans for UI, monospace for numbers
// and addresses. Bound to the CSS variables that `packages/config/tailwind.js`
// references under `fontFamily.sans` / `fontFamily.mono`.
const interTight = Inter_Tight({
  variable: "--font-inter-tight",
  subsets: ["latin"],
  display: "swap",
});

const jetbrainsMono = JetBrains_Mono({
  variable: "--font-jetbrains-mono",
  subsets: ["latin"],
  display: "swap",
});

export const metadata: Metadata = {
  title: "Aegis — AI Portfolio Manager",
  description:
    "AI-powered adaptive crypto portfolio management. Autonomously monitors, rebalances, and explains your investments.",
  keywords: ["crypto", "portfolio", "AI", "rebalancing", "DeFi"],
  robots: { index: false, follow: false },
  openGraph: {
    title: "Aegis — AI Portfolio Manager",
    description: "AI-powered adaptive crypto portfolio management.",
    type: "website",
  },
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body
        className={`${interTight.variable} ${jetbrainsMono.variable} antialiased font-sans`}
      >
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
