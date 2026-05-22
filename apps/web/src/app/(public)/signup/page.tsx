import type { Metadata } from "next";
import { redirect } from "next/navigation";

export const metadata: Metadata = {
  title: "Aegis · Continue",
  robots: "noindex, nofollow",
};

export default function SignupPage({
  searchParams,
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  return redirectToLogin(searchParams);
}

async function redirectToLogin(
  searchParams?: Promise<Record<string, string | string[] | undefined>>,
) {
  const forwardSearch = new URLSearchParams();
  const params = await searchParams;

  if (params) {
    for (const [key, value] of Object.entries(params)) {
      if (value === undefined) continue;
      if (Array.isArray(value)) {
        for (const next of value) {
          if (next !== undefined) forwardSearch.set(key, next);
        }
      } else {
        forwardSearch.set(key, value);
      }
    }
  }

  const query = forwardSearch.toString();
  redirect(`/login${query ? `?${query}` : ""}`);
}
