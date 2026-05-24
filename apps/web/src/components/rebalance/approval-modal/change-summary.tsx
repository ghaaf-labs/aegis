import {
  chainDisplayName,
  destinationActionLabel,
  sourceActionLabel,
} from "./helpers";

export function ChangeSummary({
  changeHeadline,
  sources,
  destinations,
  bridgedUsdc,
  isMockExecution,
  bridgeSourceChain,
  bridgeTargetChain,
}: {
  changeHeadline: string;
  sources: Array<{ symbol: string; amountUsdc: number }>;
  destinations: Array<{ symbol: string; amountUsdc: number }>;
  bridgedUsdc: number;
  isMockExecution: boolean;
  bridgeSourceChain: "arc" | "base";
  bridgeTargetChain: "arc" | "base";
}) {
  return (
    <div className="mb-4 border-2 border-accent-agent/30 bg-cyan-500/5 p-4">
      <p className="text-[10px] font-mono uppercase tracking-wider text-accent-agent">
        What will change
      </p>
      <h3 className="mt-1 text-lg font-semibold text-text-hi">
        {changeHeadline}
      </h3>
      <div className="mt-3 grid gap-2 text-xs font-mono text-text-lo">
        {sources.map((item) => (
          <div
            key={`source-${item.symbol}`}
            className="flex items-center justify-between border border-risk/20 bg-risk/5 px-3 py-2 text-risk"
          >
            <span>{sourceActionLabel(item.symbol)}</span>
            <span>${item.amountUsdc.toFixed(2)}</span>
          </div>
        ))}
        {destinations.length > 0 ? (
          destinations.map((item) => (
            <div
              key={`dest-${item.symbol}`}
              className="flex items-center justify-between border border-white/10 bg-black/30 px-3 py-2"
            >
              <span>{destinationActionLabel(item.symbol)}</span>
              <span className="text-accent-pnl">
                ${item.amountUsdc.toFixed(2)}
              </span>
            </div>
          ))
        ) : (
          <div className="border border-white/10 bg-black/30 px-3 py-2">
            No buy or park leg is needed. The plan only moves existing exposure.
          </div>
        )}
        {bridgedUsdc > 0 && (
          <div className="flex items-center justify-between border border-cyan-500/20 bg-cyan-500/5 px-3 py-2 text-accent-agent">
            <span>
              {isMockExecution ? "Bridge preview" : "Bridge"}{" "}
              {chainDisplayName(bridgeSourceChain)} →{" "}
              {chainDisplayName(bridgeTargetChain)}
            </span>
            <span>${bridgedUsdc.toFixed(2)}</span>
          </div>
        )}
      </div>
    </div>
  );
}
