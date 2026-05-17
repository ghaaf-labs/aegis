"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { getToken } from "@/lib/api";

export function DashboardLink() {
  const [show, setShow] = useState(false);
  useEffect(() => {
    setShow(getToken() !== null);
  }, []);
  if (!show) return null;
  return (
    <Link
      href="/dashboard"
      className="text-accent-agent hover:underline text-xs font-mono"
    >
      ← Back to dashboard
    </Link>
  );
}
