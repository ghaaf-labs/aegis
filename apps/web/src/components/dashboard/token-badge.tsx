import type { SVGProps } from "react";

interface TokenBadgeProps extends SVGProps<SVGSVGElement> {
  symbol: string;
}

interface TokenStyle {
  fill: string;
  stroke: string;
  text: string;
  mark: string;
  kind?: "text" | "diamond";
}

const TOKEN_STYLES: Record<string, TokenStyle> = {
  BTC: {
    fill: "#18110A",
    stroke: "#F7931A",
    text: "#F7931A",
    mark: "₿",
  },
  cbBTC: {
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
  SOL: {
    fill: "#130B1D",
    stroke: "#9945FF",
    text: "#14F195",
    mark: "S",
  },
  LINK: {
    fill: "#071328",
    stroke: "#2A5ADA",
    text: "#6EA8FF",
    mark: "L",
  },
  UNI: {
    fill: "#210614",
    stroke: "#FF007A",
    text: "#FF75B8",
    mark: "U",
  },
  AVAX: {
    fill: "#220909",
    stroke: "#E84142",
    text: "#FF8F8F",
    mark: "A",
  },
  USDC: {
    fill: "#071328",
    stroke: "#2775CA",
    text: "#6AB7FF",
    mark: "$",
  },
  USYC: {
    fill: "#101807",
    stroke: "#00FF88",
    text: "#00FF88",
    mark: "Y",
  },
};

export function TokenBadge({ symbol, className, ...props }: TokenBadgeProps) {
  const style = tokenStyle(symbol);

  return (
    <svg
      className={className ?? "h-7 w-7 shrink-0"}
      viewBox="0 0 28 28"
      role="img"
      aria-label={`${symbol} token`}
      {...props}
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

function tokenStyle(symbol: string) {
  return (
    TOKEN_STYLES[symbol] ?? {
      fill: "#061820",
      stroke: "#00E0FF",
      text: "#68E9FF",
      mark: symbol.slice(0, 1).toUpperCase(),
    }
  );
}
