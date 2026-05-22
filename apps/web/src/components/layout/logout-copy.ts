export function logoutFailureMessage(error: unknown) {
  const message = error instanceof Error ? error.message.toLowerCase() : "";
  if (message.includes("still accepts")) {
    return "Sign out is still finishing. Try again.";
  }
  if (message.includes("verification failed")) {
    return "We could not finish signing you out. Try again.";
  }
  return "We could not sign you out. Check your connection and try again.";
}

export function logoutRedirect() {
  const params = new URLSearchParams({ signedOut: "1" });
  return `/login?${params.toString()}`;
}
