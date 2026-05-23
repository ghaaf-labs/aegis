import Link from "next/link";

export default function NotFound() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center bg-[#0A0A0A] px-6 text-center text-text-hi">
      <p className="font-mono text-[11px] uppercase tracking-widest text-accent-agent">
        Error 404
      </p>
      <h1 className="mt-2 font-mono text-4xl font-bold tracking-tight">
        Page not found
      </h1>
      <p className="mt-3 max-w-sm font-mono text-sm text-text-lo">
        This page doesn&apos;t exist or has moved.
      </p>
      <Link
        href="/"
        className="mt-6 inline-flex min-h-11 items-center justify-center rounded-sharp border-brutal border-black bg-accent-agent px-5 font-mono font-semibold text-black shadow-brutal-sm hover:shadow-brutal"
      >
        Back to Aegis
      </Link>
    </main>
  );
}
