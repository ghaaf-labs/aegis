export function DashboardTopology() {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 760 260"
      className="absolute inset-y-0 right-0 hidden h-full w-[62%] min-w-[560px] text-accent-agent/70 md:block"
      preserveAspectRatio="xMidYMid slice"
    >
      <defs>
        <linearGradient id="topologyFade" x1="0" x2="1" y1="0" y2="0">
          <stop offset="0%" stopColor="#0a0a0a" stopOpacity="0" />
          <stop offset="36%" stopColor="#0a0a0a" stopOpacity="0.28" />
          <stop offset="100%" stopColor="#0a0a0a" stopOpacity="0.86" />
        </linearGradient>
        <pattern
          id="topologyGrid"
          width="28"
          height="28"
          patternUnits="userSpaceOnUse"
        >
          <path
            d="M 28 0 L 0 0 0 28"
            fill="none"
            stroke="rgba(255,255,255,0.055)"
            strokeWidth="1"
          />
        </pattern>
      </defs>

      <rect width="760" height="260" fill="url(#topologyGrid)" />
      <rect width="760" height="260" fill="url(#topologyFade)" />

      <g
        fill="none"
        stroke="rgba(255,255,255,0.22)"
        strokeWidth="1"
        vectorEffect="non-scaling-stroke"
      >
        <path d="M62 55h146l28 28h112" />
        <path d="M62 204h146l28-28h112" />
        <path d="M402 78h68l28 34h96" />
        <path d="M402 176h68l28-34h96" />
        <path d="M350 130h244" />
      </g>

      <g
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        vectorEffect="non-scaling-stroke"
      >
        <path d="M242 82c58 0 78 48 118 48" />
        <path d="M242 178c58 0 78-48 118-48" />
        <path d="M518 112c24 0 34-18 62-18h76" />
        <path d="M518 148c24 0 34 18 62 18h76" />
      </g>

      <g className="text-accent-pnl">
        <RailCard x={118} y={42} tone="pnl" />
        <RailCard x={118} y={178} tone="pnl" />
        <Node x={262} y={130} tone="pnl" />
        <Node x={404} y={82} tone="pnl" />
        <Node x={404} y={178} tone="pnl" />
      </g>

      <g>
        <Node x={404} y={130} tone="agent" />
        <Switch x={510} y={108} />
        <Switch x={510} y={134} />
      </g>

      <g>
        <PositionCard x={624} y={56} tone="pnl" />
        <PositionCard x={624} y={116} tone="agent" />
        <PositionCard x={624} y={176} tone="pnl" />
      </g>

      <g fill="rgba(255,176,32,0.86)">
        <rect x="708" y="76" width="4" height="4" />
        <rect x="708" y="136" width="4" height="4" />
        <rect x="708" y="196" width="4" height="4" />
      </g>
    </svg>
  );
}

function RailCard({ x, y }: { x: number; y: number; tone: "pnl" | "agent" }) {
  return (
    <g transform={`translate(${x} ${y})`}>
      <rect
        width="96"
        height="40"
        fill="rgba(10,10,10,0.78)"
        stroke="rgba(255,255,255,0.18)"
      />
      <rect x="12" y="10" width="34" height="5" fill="currentColor" />
      <rect x="12" y="22" width="58" height="4" fill="rgba(255,255,255,0.16)" />
      <rect
        x="76"
        y="10"
        width="10"
        height="20"
        fill="currentColor"
        opacity="0.75"
      />
    </g>
  );
}

function Node({ x, y, tone }: { x: number; y: number; tone: "pnl" | "agent" }) {
  const className = tone === "pnl" ? "text-accent-pnl" : "text-accent-agent";
  return (
    <g className={className} transform={`translate(${x} ${y})`}>
      <circle r="18" fill="rgba(10,10,10,0.82)" stroke="currentColor" />
      <circle r="7" fill="currentColor" />
      <circle r="26" fill="none" stroke="currentColor" opacity="0.24" />
    </g>
  );
}

function Switch({ x, y }: { x: number; y: number }) {
  return (
    <g transform={`translate(${x} ${y})`}>
      <rect
        width="50"
        height="18"
        fill="rgba(10,10,10,0.82)"
        stroke="rgba(255,255,255,0.18)"
      />
      <rect x="7" y="6" width="5" height="5" fill="currentColor" />
      <rect x="20" y="6" width="5" height="5" fill="currentColor" />
      <rect x="33" y="6" width="5" height="5" fill="currentColor" />
    </g>
  );
}

function PositionCard({
  x,
  y,
  tone,
}: {
  x: number;
  y: number;
  tone: "pnl" | "agent";
}) {
  const className = tone === "pnl" ? "text-accent-pnl" : "text-accent-agent";
  return (
    <g className={className} transform={`translate(${x} ${y})`}>
      <rect
        width="92"
        height="36"
        fill="rgba(10,10,10,0.8)"
        stroke="rgba(255,255,255,0.2)"
      />
      <rect x="10" y="9" width="36" height="5" fill="currentColor" />
      <rect x="10" y="20" width="54" height="4" fill="rgba(255,255,255,0.16)" />
      <rect
        x="70"
        y="8"
        width="12"
        height="20"
        fill="currentColor"
        opacity="0.68"
      />
    </g>
  );
}
