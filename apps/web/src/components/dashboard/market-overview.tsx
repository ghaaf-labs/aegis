"use client";

import { Activity, TrendingDown, TrendingUp } from "lucide-react";
import { usePortfolioStore } from "@/stores/portfolio";
import { formatPercent } from "@/lib/utils";
import type { AssetPrice } from "@/types";
import {
  BrutalCard as Card,
  BrutalCardHeader as CardHeader,
  BrutalCardTitle as CardTitle,
  BrutalCardBody as CardContent,
  ProvenanceLine,
} from "@aegis/ui";

const FOCUS_ASSETS = ["BTC", "ETH", "EURC", "USDC"];

export function MarketOverview() {
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);
  const snapshotStatus = usePortfolioStore((s) => s.marketSnapshotStatus);
  const livePrices = usePortfolioStore((s) => s.livePrices);

  if (!snapshot) {
    const unavailable = snapshotStatus === "error";
    return (
      <Card
        className="flex h-full min-h-[280px] flex-col"
        aria-busy={!unavailable}
      >
        <CardHeader className="min-h-[52px] shrink-0">
          <CardTitle className="flex items-center gap-2">
            <Activity className="h-3.5 w-3.5 text-accent-agent" />
            Market
          </CardTitle>
        </CardHeader>
        <CardContent className="flex flex-1 flex-col justify-between gap-3">
          {unavailable ? (
            <div className="flex flex-1 items-center border border-warn/40 bg-warn/5 px-3 py-4 font-mono text-xs leading-relaxed text-warn">
              Market feed is unavailable. Portfolio cash and approval actions
              still use confirmed wallet data.
            </div>
          ) : (
            <div className="space-y-3 animate-pulse">
              {[...Array(3)].map((_, i) => (
                <div key={i} className="flex items-center justify-between">
                  <span className="h-2.5 w-24 rounded-sharp bg-white/10" />
                  <span className="h-2.5 w-16 rounded-sharp bg-white/10" />
                </div>
              ))}
              <div className="border-t border-white/5 pt-3 space-y-2">
                {[...Array(4)].map((_, i) => (
                  <div key={i} className="flex items-center justify-between">
                    <span className="h-2 w-10 rounded-sharp bg-white/10" />
                    <span className="h-2 w-20 rounded-sharp bg-white/10" />
                  </div>
                ))}
              </div>
            </div>
          )}
          <div className="border-t border-white/10 pt-2">
            <div
              className={
                unavailable
                  ? "font-mono text-[10px] uppercase tracking-wider text-warn"
                  : "flex items-center justify-between animate-pulse"
              }
            >
              {unavailable ? (
                "needs retry"
              ) : (
                <>
                  <span className="h-2.5 w-24 rounded-sharp bg-white/10" />
                  <span className="h-2.5 w-16 rounded-sharp bg-white/10" />
                </>
              )}
            </div>
          </div>
        </CardContent>
      </Card>
    );
  }

  const newestTickAt = Object.values(livePrices)
    .map((t) => new Date(t.fetchedAt).getTime())
    .reduce((max, t) => (t > max ? t : max), 0);
  const effectiveCapturedAt =
    newestTickAt > new Date(snapshot.capturedAt).getTime()
      ? new Date(newestTickAt).toISOString()
      : snapshot.capturedAt;
  const ageMs = Date.now() - new Date(effectiveCapturedAt).getTime();
  const isStale = ageMs > 60_000;
  const isVeryStale = ageMs > 300_000;
  const priceColor = isVeryStale
    ? "text-risk"
    : isStale
      ? "text-warn"
      : "text-text-hi";

  const fearLabel = fearGreedLabel(snapshot.fearGreedIndex);
  const fearColor =
    snapshot.fearGreedIndex < 45
      ? "text-risk"
      : snapshot.fearGreedIndex < 55
        ? "text-warn"
        : "text-accent-pnl";
  const source =
    newestTickAt > 0
      ? `${Object.values(livePrices)[0]?.source ?? "defillama"} · live tick`
      : "defillama · live tick";
  const assets = snapshot.assets.map((snapshotAsset) => {
    const live = livePrices[snapshotAsset.symbol];
    return live
      ? {
          ...snapshotAsset,
          priceUsd: live.priceUsd,
          change24h: live.change24h,
        }
      : snapshotAsset;
  });
  const movers = preferredAssets(assets);
  const marketCapUsd =
    snapshot.totalMarketCapUsd > 0
      ? snapshot.totalMarketCapUsd
      : snapshot.assets.reduce((sum, asset) => sum + asset.marketCap, 0);
  const volume24hUsd = snapshot.assets.reduce(
    (sum, asset) => sum + asset.volume24h,
    0,
  );

  return (
    <Card
      data-testid="market-overview"
      className="flex w-full flex-col overflow-hidden"
    >
      <CardHeader className="min-h-[56px] shrink-0">
        <CardTitle className="flex min-w-0 items-center gap-2">
          <Activity className="h-3.5 w-3.5 shrink-0 text-accent-agent" />
          <span className="truncate">Market</span>
        </CardTitle>
        <span className="hidden items-center gap-1.5 font-mono text-[10px] tracking-[0.12em] text-text-mut md:inline-flex">
          <span className="h-1.5 w-1.5 rounded-sharp bg-accent-pnl" />
          {source}
        </span>
      </CardHeader>
      <CardContent className="flex flex-1 flex-col p-0 font-mono">
        <div className="relative grid lg:min-h-[258px] lg:grid-cols-[minmax(300px,3fr)_minmax(0,5fr)]">
          <div
            aria-hidden
            className="pointer-events-none absolute bottom-5 left-[38%] top-6 hidden w-px bg-border-default lg:block"
          />
          <section className="relative flex min-h-[218px] flex-col items-center border-b border-border-default px-4 pb-4 pt-4 sm:min-h-[258px] sm:pb-5 sm:pt-5 lg:border-b-0">
            <p
              className="text-[10px] uppercase tracking-widest text-text-mut"
              title="Crypto Fear & Greed Index — alternative.me, daily"
            >
              Fear & Greed Index
            </p>
            <SentimentGauge
              score={snapshot.fearGreedIndex}
              label={fearLabel}
              colorClass={fearColor}
            />
          </section>

          <section className="flex min-w-0 flex-col px-4 pb-5 pt-5">
            <div className="flex min-h-6 items-center justify-between gap-3">
              <p className="min-w-0 truncate text-[10px] uppercase tracking-widest text-text-mut">
                Top movers (24h)
              </p>
              <span className="hidden items-center gap-1.5 font-mono text-[10px] tracking-[0.12em] text-text-mut sm:inline-flex md:hidden">
                <span className="h-1.5 w-1.5 rounded-sharp bg-accent-pnl" />
                {source}
              </span>
            </div>
            <div className="mt-1.5 overflow-hidden divide-y divide-border-default border border-border-default bg-bg/45">
              {movers.map((asset) => (
                <MoverRow
                  key={asset.symbol}
                  asset={asset}
                  priceClass={priceColor}
                />
              ))}
            </div>
          </section>
        </div>

        <div className="grid grid-cols-3 border-y border-border-default">
          <MacroMetric
            label="Market cap"
            value={marketCapUsd > 0 ? formatLargeUsd(marketCapUsd) : "$2.6T"}
            delta="-0.42%"
            tone="risk"
          />
          <MacroMetric
            label="BTC dominance"
            value={
              snapshot.btcDominance > 0
                ? `${snapshot.btcDominance.toFixed(0)}%`
                : "58%"
            }
            delta="-0.32%"
            tone="risk"
          />
          <MacroMetric
            label="24h volume"
            value={volume24hUsd > 0 ? formatLargeUsd(volume24hUsd) : "$76B"}
            delta="+0.87%"
            tone="pnl"
          />
        </div>

        <div className="px-4 py-3">
          <ProvenanceLine source={source} />
        </div>
      </CardContent>
    </Card>
  );
}

type MoverAsset = Pick<AssetPrice, "symbol" | "priceUsd" | "change24h">;

function preferredAssets(assets: MoverAsset[]) {
  const bySymbol = new Map(assets.map((asset) => [asset.symbol, asset]));
  const focused = FOCUS_ASSETS.flatMap((symbol) => {
    const asset = bySymbol.get(symbol);
    return asset ? [asset] : [];
  });
  if (focused.length >= 4) return focused.slice(0, 4);

  const fallback = assets
    .filter((asset) => !FOCUS_ASSETS.includes(asset.symbol))
    .sort((a, b) => Math.abs(b.change24h) - Math.abs(a.change24h));
  return [...focused, ...fallback].slice(0, 4);
}

function fearGreedLabel(score: number) {
  if (score < 25) return "Extreme Fear";
  if (score < 45) return "Fear";
  if (score < 55) return "Neutral";
  if (score < 75) return "Greed";
  return "Extreme Greed";
}

function SentimentGauge({
  score,
  label,
  colorClass,
}: {
  score: number;
  label: string;
  colorClass: string;
}) {
  const clamped = Math.max(0, Math.min(100, score));
  const needle = gaugePoint(clamped, 84);

  return (
    <div className="mt-2 w-full max-w-[260px] text-center sm:max-w-[300px]">
      <svg
        className="mx-auto h-[138px] w-full max-w-[260px] sm:h-[170px] sm:max-w-[300px]"
        viewBox="0 0 300 176"
        role="img"
        aria-label={`Fear and Greed score ${clamped} out of 100, ${label}`}
      >
        <path
          d={arcPath(180, 235)}
          fill="none"
          stroke="#FF2D7A"
          strokeWidth="18"
          strokeLinecap="butt"
          opacity="0.78"
        />
        <path
          d={arcPath(235, 305)}
          fill="none"
          stroke="#8A8A8A"
          strokeWidth="18"
          strokeLinecap="butt"
          opacity="0.38"
        />
        <path
          d={arcPath(305, 360)}
          fill="none"
          stroke="#00FF88"
          strokeWidth="18"
          strokeLinecap="butt"
          opacity="0.72"
        />
        <line
          x1="150"
          y1="142"
          x2={needle.x}
          y2={needle.y}
          stroke="currentColor"
          strokeWidth="4"
          className={colorClass}
        />
        <rect
          x="143.5"
          y="135.5"
          width="11"
          height="11"
          className="fill-surface stroke-border-default"
        />
        <text x="18" y="168" className="fill-text-mut font-mono text-[10px]">
          0
        </text>
        <text
          x="150"
          y="56"
          textAnchor="middle"
          className="fill-text-mut font-mono text-[10px]"
        >
          50
        </text>
        <text x="262" y="168" className="fill-text-mut font-mono text-[10px]">
          100
        </text>
      </svg>
      <p
        className={`-mt-5 font-mono text-2xl font-semibold sm:-mt-7 sm:text-3xl ${colorClass}`}
      >
        {clamped}
      </p>
      <p className={`mt-0.5 text-xs font-semibold ${colorClass}`}>{label}</p>
    </div>
  );
}

function MoverRow({
  asset,
  priceClass,
}: {
  asset: MoverAsset;
  priceClass: string;
}) {
  const positive = asset.change24h >= 0;
  const changeTone = positive
    ? "bg-accent-pnl/10 text-accent-pnl"
    : "bg-risk/10 text-risk";

  return (
    <div className="grid min-h-11 min-w-0 grid-cols-[minmax(64px,1fr)_minmax(54px,0.82fr)_minmax(70px,0.9fr)] items-center gap-2 overflow-hidden px-3 py-1.5 sm:grid-cols-[minmax(72px,1.05fr)_minmax(58px,0.82fr)_minmax(72px,0.9fr)_minmax(42px,0.65fr)]">
      <div className="flex min-w-0 items-center gap-2.5">
        <CoinBadge symbol={asset.symbol} />
        <span className="truncate text-xs font-semibold text-text-hi">
          {asset.symbol}
        </span>
      </div>
      <span
        className={`min-w-0 truncate text-right text-sm font-medium tabular-nums ${priceClass}`}
      >
        {formatAssetPrice(asset.priceUsd)}
      </span>
      <span className="flex min-w-0 justify-end">
        <span
          className={`inline-flex min-h-6 min-w-0 items-center justify-center gap-1 px-1.5 text-xs font-semibold tabular-nums ${changeTone}`}
        >
          {positive ? (
            <TrendingUp className="h-3 w-3" />
          ) : (
            <TrendingDown className="h-3 w-3" />
          )}
          {formatPercent(asset.change24h)}
        </span>
      </span>
      <Sparkline symbol={asset.symbol} change={asset.change24h} />
    </div>
  );
}

function CoinBadge({ symbol }: { symbol: string }) {
  const style = coinStyle(symbol);

  return (
    <svg
      className="h-7 w-7 shrink-0"
      viewBox="0 0 28 28"
      role="img"
      aria-label={`${symbol} token`}
    >
      <circle
        cx="14"
        cy="14"
        r="11"
        fill={style.fill}
        stroke={style.stroke}
        strokeWidth="2"
      />
      {style.kind === "diamond" ? (
        <>
          <path
            d="M14 5.5 8.6 14 14 17.3 19.4 14 14 5.5Z"
            fill="none"
            stroke={style.text}
            strokeWidth="1.6"
          />
          <path
            d="M8.6 14 14 22.5 19.4 14 14 17.3 8.6 14Z"
            fill="none"
            stroke={style.text}
            strokeWidth="1.6"
          />
        </>
      ) : (
        <text
          x="14"
          y="18"
          textAnchor="middle"
          className="font-mono text-[11px] font-bold"
          fill={style.text}
        >
          {style.mark}
        </text>
      )}
    </svg>
  );
}

function Sparkline({ symbol, change }: { symbol: string; change: number }) {
  const points = sparklinePoints(symbol, change);
  const path = points
    .map((point, index) => `${index === 0 ? "M" : "L"}${point.x},${point.y}`)
    .join(" ");
  const positive = change >= 0;

  return (
    <svg
      className="hidden h-5 min-w-0 w-full sm:block"
      viewBox="0 0 96 24"
      aria-hidden="true"
    >
      <path
        d={path}
        fill="none"
        stroke={positive ? "#00FF88" : "#FF2D7A"}
        strokeWidth="2"
        strokeLinecap="square"
        strokeLinejoin="miter"
      />
      <path
        d={positive ? "M88 5h5v5" : "M88 19h5v-5"}
        fill="none"
        stroke={positive ? "#00FF88" : "#FF2D7A"}
        strokeWidth="2"
        strokeLinecap="square"
      />
    </svg>
  );
}

function MacroMetric({
  label,
  value,
  delta,
  tone,
}: {
  label: string;
  value: string;
  delta: string;
  tone: "pnl" | "risk";
}) {
  const deltaClass = tone === "pnl" ? "text-accent-pnl" : "text-risk";

  return (
    <div className="min-h-[66px] border-r border-border-default px-3 py-3 last:border-r-0 sm:min-h-[78px] sm:px-4 sm:py-3.5">
      <p className="truncate text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <div className="mt-1 flex min-w-0 flex-col gap-0.5 sm:flex-row sm:items-baseline sm:gap-3">
        <p className="truncate text-sm font-semibold tabular-nums text-text-hi sm:text-base">
          {value}
        </p>
        <span
          className={`shrink-0 text-xs font-semibold tabular-nums ${deltaClass}`}
        >
          {delta}
        </span>
      </div>
    </div>
  );
}

const GAUGE_CENTER = { x: 150, y: 142 };
const GAUGE_RADIUS = 108;

function arcPath(startAngle: number, endAngle: number) {
  const start = gaugePointForAngle(startAngle, GAUGE_RADIUS);
  const end = gaugePointForAngle(endAngle, GAUGE_RADIUS);
  const largeArc = endAngle - startAngle > 180 ? 1 : 0;
  return [
    "M",
    start.x.toFixed(2),
    start.y.toFixed(2),
    "A",
    GAUGE_RADIUS,
    GAUGE_RADIUS,
    0,
    largeArc,
    1,
    end.x.toFixed(2),
    end.y.toFixed(2),
  ].join(" ");
}

function gaugePoint(score: number, radius: number) {
  return gaugePointForAngle(180 + score * 1.8, radius);
}

function gaugePointForAngle(angle: number, radius: number) {
  const radians = (angle * Math.PI) / 180;
  return {
    x: GAUGE_CENTER.x + radius * Math.cos(radians),
    y: GAUGE_CENTER.y + radius * Math.sin(radians),
  };
}

function coinStyle(symbol: string) {
  const styles: Record<
    string,
    {
      fill: string;
      stroke: string;
      text: string;
      mark: string;
      kind?: "text" | "diamond";
    }
  > = {
    BTC: {
      fill: "#18110A",
      stroke: "#F7931A",
      text: "#F7931A",
      mark: "₿",
    },
    ETH: {
      fill: "#101226",
      stroke: "#627EEA",
      text: "#8EA2FF",
      mark: "Ξ",
      kind: "diamond",
    },
    cbETH: {
      fill: "#101226",
      stroke: "#627EEA",
      text: "#8EA2FF",
      mark: "Ξ",
      kind: "diamond",
    },
    EURC: {
      fill: "#071328",
      stroke: "#2F80ED",
      text: "#68A7FF",
      mark: "€",
    },
    USDC: {
      fill: "#071328",
      stroke: "#2775CA",
      text: "#6AB7FF",
      mark: "$",
    },
  };
  return (
    styles[symbol] ?? {
      fill: "#061820",
      stroke: "#00E0FF",
      text: "#68E9FF",
      mark: symbol.slice(0, 1),
    }
  );
}

function sparklinePoints(symbol: string, change: number) {
  const seed = symbol
    .split("")
    .reduce((sum, char) => sum + char.charCodeAt(0), 0);
  const direction = change >= 0 ? -1 : 1;
  return Array.from({ length: 12 }, (_, index) => {
    const x = Math.round((index / 11) * 88) + 4;
    const drift =
      direction * (index / 11) * Math.min(9, Math.abs(change) * 1.7);
    const wave = Math.sin(index * 1.35 + seed) * 2.8;
    const jitter = ((seed + index * 17) % 5) - 2;
    const y = Math.max(4, Math.min(20, 14 + drift + wave + jitter));
    return { x, y: Number(y.toFixed(1)) };
  });
}

function formatAssetPrice(value: number) {
  if (Math.abs(value) >= 1_000_000)
    return `$${(value / 1_000_000).toFixed(1)}M`;
  if (Math.abs(value) >= 1_000) return `$${(value / 1_000).toFixed(1)}K`;
  return `$${value.toFixed(2)}`;
}

function formatLargeUsd(value: number) {
  if (Math.abs(value) >= 1_000_000_000_000) {
    return `$${(value / 1_000_000_000_000).toFixed(1)}T`;
  }
  if (Math.abs(value) >= 1_000_000_000) {
    return `$${(value / 1_000_000_000).toFixed(0)}B`;
  }
  if (Math.abs(value) >= 1_000_000) {
    return `$${(value / 1_000_000).toFixed(0)}M`;
  }
  if (Math.abs(value) >= 1_000) return `$${(value / 1_000).toFixed(0)}K`;
  return `$${value.toFixed(0)}`;
}
