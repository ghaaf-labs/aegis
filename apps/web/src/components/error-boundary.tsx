"use client";

import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
  /** Visible label for the retry button. */
  retryLabel?: string;
}

interface State {
  error: Error | null;
}

/**
 * Catches render-time exceptions thrown by any child subtree and renders a
 * recoverable fallback. The "Try again" action remounts the subtree by
 * resetting state, which discards the failing render and lets React rebuild
 * from the latest props.
 *
 * Keep this class-based: error boundaries are the one place React still
 * requires class components. The render path is hot, so do not pull in
 * heavier dependencies (analytics, tracing) here — the catch handler emits
 * a console error and that's enough for the dev loop.
 */
export class ErrorBoundary extends Component<Props, State> {
  override state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  override componentDidCatch(error: Error, info: ErrorInfo) {
    if (typeof window !== "undefined") {
      // eslint-disable-next-line no-console
      console.error("aegis error boundary caught", error, info.componentStack);
    }
  }

  reset = () => this.setState({ error: null });

  override render() {
    if (!this.state.error) return this.props.children;

    const message =
      this.state.error.message || "An unexpected error broke this view.";

    return (
      <div className="flex items-center justify-center p-6">
        <div
          role="alert"
          className="border-2 border-rose-500/30 bg-rose-500/5 max-w-xl w-full p-6 flex flex-col gap-4"
        >
          <div>
            <p className="text-xs font-mono uppercase tracking-widest text-rose-300">
              Something broke
            </p>
            <p className="mt-1 text-sm text-text-hi font-mono break-words">
              {message}
            </p>
          </div>
          <button
            type="button"
            onClick={this.reset}
            className="self-start px-3 py-1.5 text-xs font-mono uppercase tracking-widest border-2 border-cyan-500/40 text-cyan-300 bg-cyan-500/10 hover:bg-cyan-500/20"
          >
            {this.props.retryLabel ?? "Try again"}
          </button>
        </div>
      </div>
    );
  }
}
