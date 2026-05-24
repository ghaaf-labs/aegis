import Link from "next/link";

export default function DemoNotFound() {
  return (
    <div className="max-w-[1400px] mx-auto flex min-h-[60vh] flex-col items-center justify-center space-y-6 text-center">
      <p className="font-mono text-[10px] uppercase tracking-widest text-accent-agent">
        404 · Demo not found
      </p>
      <h1 className="text-3xl font-mono font-semibold text-text-hi tracking-tight">
        Demo not found
      </h1>
      <p className="max-w-sm text-sm font-mono text-text-lo leading-relaxed">
        That demo portfolio slug doesn&apos;t exist. Try one of the curated
        demos below.
      </p>
      <Link
        href="/explore"
        className="inline-flex min-h-10 items-center gap-2 border border-accent-agent/40 bg-accent-agent/10 px-5 py-2 font-mono text-sm text-accent-agent hover:border-accent-agent"
      >
        Back to demo portfolios
      </Link>
    </div>
  );
}
