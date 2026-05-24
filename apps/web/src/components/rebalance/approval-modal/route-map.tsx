import type { RebalancePlanResponse } from "@/lib/api";
import {
  bridgedAmountUsdc,
  chainAmount,
  chainDestinationTotals,
  chainDisplayName,
  chainLabel,
  chainPositionSaleTotals,
  chainSourceTotals,
  destinationAmounts,
  normalizeRouteChain,
  sourceAmounts,
} from "./helpers";

export function RebalanceRouteMap({ plan }: { plan: RebalancePlanResponse }) {
  const bridged = bridgedAmountUsdc(plan);
  const sourceTotals = chainSourceTotals(plan);
  const saleTotals = chainPositionSaleTotals(plan);
  const targetTotals = chainDestinationTotals(plan);
  const targets = destinationAmounts(plan).slice(0, 4);
  const hasPositionSales = sourceAmounts(plan).length > 0;
  const hasBridge = bridged > 0;
  const bridgeLeg = plan.legs.find((leg) => leg.kind === "cross_chain_burn");
  const sourceChain = normalizeRouteChain(bridgeLeg?.srcChain ?? "arc");
  const targetChain = normalizeRouteChain(bridgeLeg?.destChain ?? "base");
  const sourceUsd = hasPositionSales
    ? chainAmount(saleTotals, sourceChain)
    : chainAmount(sourceTotals, sourceChain);
  const targetUsd = chainAmount(targetTotals, targetChain);

  if (!hasBridge) {
    const chain = normalizeRouteChain(
      plan.legs[0]?.srcChain ?? plan.legs[0]?.destChain ?? "arc",
    );
    return (
      <SingleChainRouteMap
        chain={chain}
        sourceUsd={
          hasPositionSales
            ? chainAmount(saleTotals, chain)
            : chainAmount(sourceTotals, chain)
        }
        targetUsd={chainAmount(targetTotals, chain)}
        targets={targets}
        sourceKind={hasPositionSales ? "positions" : "wallet"}
      />
    );
  }

  return (
    <div className="mb-4 border border-white/10 bg-black/30 p-3">
      <div className="mb-2 flex items-center justify-between gap-3">
        <p className="text-[10px] font-mono uppercase tracking-wider text-accent-agent">
          Money path
        </p>
        <p className="text-[10px] font-mono text-text-mut">
          {chainDisplayName(sourceChain)} → {chainDisplayName(targetChain)}
        </p>
      </div>
      <svg
        viewBox="0 0 560 170"
        role="img"
        aria-label={`Route map showing ${chainLabel(sourceChain)} source cash, CCTP bridge, ${chainLabel(targetChain)} target exposure, and target assets`}
        className="h-auto w-full"
      >
        <rect
          x="1"
          y="1"
          width="558"
          height="168"
          fill="#0A0A0A"
          stroke="#2A2A2A"
          strokeWidth="2"
        />
        <g>
          <rect
            x="22"
            y="34"
            width="132"
            height="78"
            fill="#101010"
            stroke={hasPositionSales ? "#fb7185" : "#38E27D"}
            strokeWidth="2"
          />
          <text
            x="38"
            y="61"
            fill={hasPositionSales ? "#fb7185" : "#38E27D"}
            fontFamily="monospace"
            fontSize="12"
            fontWeight="700"
          >
            {hasPositionSales ? "Sold positions" : "Wallet cash"}
          </text>
          <text
            x="38"
            y="86"
            fill="#E8E8E8"
            fontFamily="monospace"
            fontSize="18"
          >
            ${sourceUsd.toFixed(2)}
          </text>
          <text
            x="38"
            y="103"
            fill="#8A8A8A"
            fontFamily="monospace"
            fontSize="10"
          >
            {hasPositionSales
              ? "changed to USDC"
              : chainDisplayName(sourceChain)}
          </text>
        </g>

        <path
          d="M158 73H242"
          fill="none"
          stroke={hasBridge ? "#55D7FF" : "#3A3A3A"}
          strokeWidth="3"
          strokeDasharray={hasBridge ? "8 6" : "0"}
        >
          {hasBridge && (
            <animate
              attributeName="stroke-dashoffset"
              from="0"
              to="-28"
              dur="1.5s"
              repeatCount="indefinite"
            />
          )}
        </path>
        <g>
          <rect
            x="232"
            y="45"
            width="96"
            height="56"
            fill="#061318"
            stroke="#55D7FF"
            strokeWidth="2"
          />
          <text
            x="280"
            y="69"
            textAnchor="middle"
            fill="#55D7FF"
            fontFamily="monospace"
            fontSize="11"
            fontWeight="700"
          >
            CCTP V2
          </text>
          <text
            x="280"
            y="88"
            textAnchor="middle"
            fill="#E8E8E8"
            fontFamily="monospace"
            fontSize="13"
          >
            ${bridged.toFixed(2)}
          </text>
        </g>
        <path
          d="M328 73H406"
          fill="none"
          stroke={hasBridge ? "#55D7FF" : "#3A3A3A"}
          strokeWidth="3"
          strokeDasharray={hasBridge ? "8 6" : "0"}
        >
          {hasBridge && (
            <animate
              attributeName="stroke-dashoffset"
              from="0"
              to="-28"
              dur="1.5s"
              repeatCount="indefinite"
            />
          )}
        </path>

        <g>
          <rect
            x="406"
            y="34"
            width="132"
            height="78"
            fill="#101010"
            stroke="#38E27D"
            strokeWidth="2"
          />
          <text
            x="422"
            y="61"
            fill="#38E27D"
            fontFamily="monospace"
            fontSize="12"
            fontWeight="700"
          >
            Target mix
          </text>
          <text
            x="422"
            y="86"
            fill="#E8E8E8"
            fontFamily="monospace"
            fontSize="18"
          >
            ${targetUsd.toFixed(2)}
          </text>
          <text
            x="422"
            y="103"
            fill="#8A8A8A"
            fontFamily="monospace"
            fontSize="10"
          >
            final exposure
          </text>
        </g>

        <g transform="translate(24 130)">
          {targets.map((target, index) => (
            <g key={target.symbol} transform={`translate(${index * 132} 0)`}>
              <rect
                width="116"
                height="24"
                fill="#151515"
                stroke="#2A2A2A"
                strokeWidth="1"
              />
              <text
                x="9"
                y="16"
                fill="#E8E8E8"
                fontFamily="monospace"
                fontSize="10"
                fontWeight="700"
              >
                {target.symbol}
              </text>
              <text
                x="107"
                y="16"
                textAnchor="end"
                fill="#38E27D"
                fontFamily="monospace"
                fontSize="10"
              >
                ${target.amountUsdc.toFixed(0)}
              </text>
            </g>
          ))}
        </g>
      </svg>
    </div>
  );
}

function SingleChainRouteMap({
  chain,
  sourceUsd,
  targetUsd,
  targets,
  sourceKind,
}: {
  chain: "arc" | "base";
  sourceUsd: number;
  targetUsd: number;
  targets: Array<{ symbol: string; amountUsdc: number }>;
  sourceKind: "wallet" | "positions";
}) {
  return (
    <div className="mb-4 border border-white/10 bg-black/30 p-3 font-mono">
      <div className="mb-2 flex items-center justify-between gap-3">
        <p className="text-[10px] font-mono uppercase tracking-wider text-accent-agent">
          Money path
        </p>
        <p className="text-[10px] font-mono text-text-mut">
          {chainDisplayName(chain)} only
        </p>
      </div>
      <div
        role="img"
        aria-label={`Money path showing ${chainDisplayName(chain)} wallet cash moving into the target allocation`}
        className="border border-white/10 bg-bg p-3"
      >
        <div className="grid items-stretch gap-3 sm:grid-cols-[minmax(0,1fr)_64px_minmax(0,1fr)]">
          <RouteNode
            label={
              sourceKind === "positions" ? "Sold positions" : "Wallet cash"
            }
            value={`$${sourceUsd.toFixed(2)}`}
            detail={
              sourceKind === "positions"
                ? "changed to USDC"
                : chainDisplayName(chain)
            }
            tone={sourceKind === "positions" ? "risk" : "pnl"}
          />
          <div className="flex h-8 items-center justify-center self-center sm:h-auto sm:self-stretch">
            <div className="h-full w-px border-l-2 border-dashed border-accent-pnl sm:h-px sm:w-full sm:border-l-0 sm:border-t-2" />
          </div>
          <RouteNode
            label="Target mix"
            value={`$${targetUsd.toFixed(2)}`}
            detail="no bridge needed"
            tone="pnl"
          />
        </div>

        <div className="mt-3 grid gap-2 sm:grid-cols-2">
          {targets.map((target) => (
            <RouteChip
              key={target.symbol}
              label={target.symbol}
              value={`$${target.amountUsdc.toFixed(0)}`}
            />
          ))}
          <RouteChip label="USDC reserve" value="cash" tone="pnl" />
        </div>
      </div>
    </div>
  );
}

function RouteNode({
  label,
  value,
  detail,
  tone,
}: {
  label: string;
  value: string;
  detail: string;
  tone: "pnl" | "risk";
}) {
  const toneClass = tone === "risk" ? "text-risk" : "text-accent-pnl";
  const borderClass =
    tone === "risk" ? "border-risk/60" : "border-accent-pnl/60";
  return (
    <div
      data-route-node="true"
      className={`flex min-h-24 flex-col justify-center border-2 bg-surface px-4 py-3 ${borderClass}`}
    >
      <p className={`text-[10px] font-semibold ${toneClass}`}>{label}</p>
      <p className="mt-2 text-xl text-text-hi">{value}</p>
      <p className="mt-1 text-[10px] text-text-mut">{detail}</p>
    </div>
  );
}

function RouteChip({
  label,
  value,
  tone = "default",
}: {
  label: string;
  value: string;
  tone?: "default" | "pnl";
}) {
  return (
    <div
      className={
        "grid min-h-8 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 border px-3 py-1.5 text-[10px] " +
        (tone === "pnl"
          ? "border-accent-pnl/45 bg-accent-pnl/5 text-accent-pnl"
          : "border-white/10 bg-black/30 text-text-hi")
      }
    >
      <span className="min-w-0 truncate font-semibold">{label}</span>
      <span className={tone === "pnl" ? "text-accent-pnl" : "text-accent-pnl"}>
        {value}
      </span>
    </div>
  );
}
