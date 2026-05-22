export function DashboardTopology() {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 820 260"
      className="absolute inset-y-0 right-0 hidden h-full w-[64%] min-w-[620px] text-accent-agent/70 md:block"
      preserveAspectRatio="xMidYMid slice"
    >
      <defs>
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

      <rect width="820" height="260" fill="#0A0A0A" />
      <rect width="820" height="260" fill="url(#topologyGrid)" />
      <rect
        x="0"
        y="0"
        width="250"
        height="260"
        fill="#0A0A0A"
        opacity="0.88"
      />
      <rect
        x="250"
        y="0"
        width="190"
        height="260"
        fill="#0A0A0A"
        opacity="0.68"
      />
      <rect
        x="440"
        y="0"
        width="380"
        height="260"
        fill="#0A0A0A"
        opacity="0.42"
      />

      <g
        fill="none"
        stroke="rgba(255,255,255,0.2)"
        strokeWidth="1"
        vectorEffect="non-scaling-stroke"
      >
        <path d="M74 80h120l24 50h70" />
        <path d="M74 180h120l24-50h70" />
        <path d="M438 130h66l34-50h84" />
        <path d="M438 130h66l34 50h84" />
        <path d="M438 130h184" />
      </g>

      <g
        fill="none"
        stroke="currentColor"
        strokeWidth="3"
        strokeDasharray="11 9"
        vectorEffect="non-scaling-stroke"
      >
        <path d="M194 80h40l44 50h54">
          <AnimateRail active />
        </path>
        <path d="M194 180h40l44-50h54">
          <AnimateRail active />
        </path>
        <path d="M468 130h66l34-50h54">
          <AnimateRail active />
        </path>
        <path d="M468 130h66l34 50h54">
          <AnimateRail active />
        </path>
      </g>

      <SystemCard x={64} y={50} label="ARC" value="USDC" tone="pnl" />
      <SystemCard x={64} y={150} label="BASE" value="USDC" tone="pnl" />
      <GateCard x={302} y={88} />
      <SystemCard x={632} y={50} label="USYC" value="yield" tone="pnl" />
      <SystemCard x={632} y={112} label="EURC" value="fx" tone="pnl" />
      <SystemCard x={632} y={174} label="USDC" value="reserve" tone="pnl" />

      <DecisionNode x={224} y={130} tone="pnl" />
      <DecisionNode x={468} y={130} tone="agent" />
      <ApprovalStamp x={540} y={104} />
    </svg>
  );
}

function SystemCard({
  x,
  y,
  label,
  value,
  tone,
}: {
  x: number;
  y: number;
  label: string;
  value: string;
  tone: "pnl" | "agent";
}) {
  const className = tone === "pnl" ? "text-accent-pnl" : "text-accent-agent";
  return (
    <g className={className} transform={`translate(${x} ${y})`}>
      <rect
        width="130"
        height="58"
        fill="rgba(10,10,10,0.78)"
        stroke="currentColor"
        strokeWidth="1.5"
      />
      <rect
        x="10"
        y="10"
        width="4"
        height="38"
        fill="currentColor"
        opacity="0.82"
      />
      <text
        x="22"
        y="27"
        fill="#FFFFFF"
        fontFamily="monospace"
        fontSize="13"
        fontWeight="700"
      >
        {label}
      </text>
      <text x="22" y="44" fill="#8A8A8A" fontFamily="monospace" fontSize="10">
        {value}
      </text>
      <rect
        x="96"
        y="15"
        width="20"
        height="8"
        fill="currentColor"
        opacity="0.8"
      />
      <rect
        x="96"
        y="31"
        width="12"
        height="8"
        fill="currentColor"
        opacity="0.35"
      />
    </g>
  );
}

function GateCard({ x, y }: { x: number; y: number }) {
  return (
    <g transform={`translate(${x} ${y})`}>
      <rect
        width="136"
        height="84"
        fill="rgba(10,10,10,0.92)"
        stroke="#00E0FF"
        strokeWidth="2"
      />
      <text
        x="14"
        y="24"
        fill="#00E0FF"
        fontFamily="monospace"
        fontSize="11"
        fontWeight="700"
      >
        AGENT REVIEW
      </text>
      <text
        x="14"
        y="44"
        fill="#FFFFFF"
        fontFamily="monospace"
        fontSize="12"
        fontWeight="700"
      >
        no auto-trade
      </text>
      <text x="14" y="62" fill="#8A8A8A" fontFamily="monospace" fontSize="10">
        approve plan first
      </text>
      <rect
        x="104"
        y="14"
        width="16"
        height="56"
        fill="#00E0FF"
        opacity="0.18"
      />
      <rect x="110" y="22" width="4" height="40" fill="#00E0FF" />
    </g>
  );
}

function DecisionNode({
  x,
  y,
  tone,
}: {
  x: number;
  y: number;
  tone: "pnl" | "agent";
}) {
  const color = tone === "pnl" ? "#00FF88" : "#00E0FF";
  return (
    <g transform={`translate(${x} ${y})`}>
      <rect
        x="-26"
        y="-26"
        width="52"
        height="52"
        fill="#0A0A0A"
        stroke={color}
        strokeWidth="2"
      />
      <rect x="-9" y="-9" width="18" height="18" fill={color} />
      <path d="M-14 36H14M-14 43H8" stroke={color} strokeWidth="2" />
    </g>
  );
}

function ApprovalStamp({ x, y }: { x: number; y: number }) {
  return (
    <g transform={`translate(${x} ${y})`}>
      <rect
        width="76"
        height="52"
        fill="rgba(255,184,0,0.08)"
        stroke="#FFB800"
        strokeWidth="2"
      />
      <text
        x="38"
        y="22"
        fill="#FFB800"
        fontFamily="monospace"
        fontSize="10"
        fontWeight="700"
        textAnchor="middle"
      >
        USER
      </text>
      <text
        x="38"
        y="38"
        fill="#FFB800"
        fontFamily="monospace"
        fontSize="10"
        fontWeight="700"
        textAnchor="middle"
      >
        APPROVAL
      </text>
    </g>
  );
}

function AnimateRail({ active }: { active: boolean }) {
  if (!active) return null;
  return (
    <animate
      attributeName="stroke-dashoffset"
      dur="2.8s"
      from="40"
      repeatCount="indefinite"
      to="0"
    />
  );
}
