import { ImageResponse } from "next/og";
import type { DiaryEntry } from "@/types";

export const runtime = "edge";
export const revalidate = 86400;

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

interface RouteParams {
  params: Promise<{ decisionId: string }>;
}

async function fetchDecision(id: string): Promise<DiaryEntry | null> {
  try {
    const res = await fetch(`${API_BASE}/diary/decision/${id}`, {
      next: { revalidate: 3600 },
    });
    if (!res.ok) return null;
    return (await res.json()) as DiaryEntry;
  } catch {
    return null;
  }
}

const REGIME_COLOR: Record<string, string> = {
  risk_on: "#00FF88",
  neutral: "#FFFFFF",
  risk_off: "#FF2D7A",
};

export async function GET(_req: Request, { params }: RouteParams) {
  const { decisionId } = await params;
  const entry = await fetchDecision(decisionId);

  if (!entry) {
    return new Response("Decision not found", { status: 404 });
  }

  const regimeColor = entry.regime
    ? (REGIME_COLOR[entry.regime] ?? "#fff")
    : "#fff";
  const realized = entry.outcome?.realizedPctChange;
  const realizedStr =
    realized === undefined
      ? "—"
      : `${realized >= 0 ? "+" : ""}${realized.toFixed(2)}%`;
  const realizedColor =
    realized === undefined ? "#888" : realized >= 0 ? "#00FF88" : "#FF2D7A";

  return new ImageResponse(
    <div
      style={{
        width: "100%",
        height: "100%",
        background: "#0A0A0A",
        color: "#fff",
        padding: "64px",
        display: "flex",
        flexDirection: "column",
        justifyContent: "space-between",
        fontFamily: "system-ui, sans-serif",
      }}
    >
      <div style={{ display: "flex", gap: 16, alignItems: "center" }}>
        <div
          style={{
            background: "#00E0FF",
            color: "#000",
            fontWeight: 800,
            padding: "8px 16px",
            fontSize: 20,
            letterSpacing: "0.04em",
            textTransform: "uppercase",
          }}
        >
          Aegis
        </div>
        {entry.regime && (
          <div
            style={{
              background: regimeColor,
              color: "#000",
              padding: "8px 16px",
              fontSize: 18,
              fontWeight: 700,
              textTransform: "uppercase",
            }}
          >
            {entry.regime.replace("_", "-")}
          </div>
        )}
        {entry.modelSlug && (
          <div
            style={{
              background: "#111",
              color: "#00E0FF",
              padding: "6px 12px",
              fontSize: 14,
              fontFamily: "monospace",
              border: "1px solid #00E0FF",
            }}
          >
            {entry.modelSlug}
          </div>
        )}
        {entry.criticVerdict && (
          <div
            style={{
              background:
                entry.criticVerdict.verdict === "revised"
                  ? "#FF2D7A"
                  : "#00FF88",
              color: "#000",
              padding: "6px 12px",
              fontSize: 14,
              fontWeight: 700,
              textTransform: "uppercase",
            }}
          >
            CRITIC:{" "}
            {entry.criticVerdict.verdict === "revised" ? "REVISED" : "APPROVED"}
          </div>
        )}
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
        <div
          style={{
            fontSize: 44,
            fontWeight: 700,
            lineHeight: 1.1,
            maxWidth: 1072,
          }}
        >
          {entry.recommendationSummary}
        </div>
        <div style={{ display: "flex", gap: 48 }}>
          <div style={{ display: "flex", flexDirection: "column" }}>
            <div style={{ fontSize: 14, color: "#888" }}>Realized 24h</div>
            <div
              style={{
                fontSize: 48,
                fontFamily: "monospace",
                color: realizedColor,
              }}
            >
              {realizedStr}
            </div>
          </div>
          <div style={{ display: "flex", flexDirection: "column" }}>
            <div style={{ fontSize: 14, color: "#888" }}>Model</div>
            <div style={{ fontSize: 28, fontFamily: "monospace" }}>
              {entry.modelSlug ?? "n/a"}
            </div>
          </div>
          <div style={{ display: "flex", flexDirection: "column" }}>
            <div style={{ fontSize: 14, color: "#888" }}>Confidence</div>
            <div style={{ fontSize: 28, fontFamily: "monospace" }}>
              {Math.round(entry.confidence * 100)}%
            </div>
          </div>
        </div>
      </div>

      <div
        style={{
          fontFamily: "monospace",
          fontSize: 16,
          color: "#888",
          display: "flex",
          justifyContent: "space-between",
        }}
      >
        <span>Aegis · adaptive crypto portfolio</span>
        <span>Arc + Base · CCTP V2</span>
      </div>
    </div>,
    { width: 1200, height: 630 },
  );
}
