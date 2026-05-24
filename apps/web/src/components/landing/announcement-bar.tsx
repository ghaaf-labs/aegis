"use client";

import { useState } from "react";
import { ExternalLink, Trophy, X } from "lucide-react";

export function AnnouncementBar() {
  const [announcementDismissed, setAnnouncementDismissed] = useState(() => {
    if (typeof window === "undefined") return false;
    return sessionStorage.getItem("aegis.announcement.dismissed") === "1";
  });

  if (announcementDismissed) return null;

  return (
    <div className="relative z-20 flex items-center justify-center gap-3 px-4 py-2 bg-accent-agent/10 border-b border-accent-agent/20 text-xs font-mono text-accent-agent">
      <Trophy className="w-3.5 h-3.5 shrink-0" />
      <span>
        Built for <span className="font-semibold">Agora Agents Hackathon</span>{" "}
        · RFB 04 · May 11–25, 2026
      </span>
      <a
        href="https://github.com/ghaaf-labs/aegis"
        target="_blank"
        rel="noopener noreferrer"
        className="flex items-center gap-1 underline underline-offset-2 hover:text-accent-agent/80"
      >
        View on GitHub <ExternalLink className="w-3 h-3" />
      </a>
      <button
        type="button"
        onClick={() => {
          sessionStorage.setItem("aegis.announcement.dismissed", "1");
          setAnnouncementDismissed(true);
        }}
        className="absolute right-4 p-1 hover:text-text-hi transition-colors"
        aria-label="Dismiss"
      >
        <X className="w-3.5 h-3.5" />
      </button>
    </div>
  );
}
