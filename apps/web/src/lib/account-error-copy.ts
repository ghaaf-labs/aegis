export function walletStatusError(error: unknown) {
  const raw =
    error instanceof Error ? error.message : "wallet status check failed";
  const message = raw.replace(/^\d{3}:\s*/, "");
  const lower = message.toLowerCase();
  if (lower.includes("missing token") || lower.includes("unauthorized")) {
    return "Your sign-in expired. Enter your email again before checking account setup.";
  }
  if (lower.includes("failed to fetch") || lower.includes("networkerror")) {
    return "We could not check your account. Check your connection and try again.";
  }
  return message;
}

export function formatGatewayBalanceError(error: unknown) {
  const raw = error instanceof Error ? error.message : "Balance check failed";
  const message = raw.replace(/^\d{3}:\s*/, "");
  const lower = message.toLowerCase();
  if (lower.includes("session expired") || lower.includes("unauthorized")) {
    return "Your sign-in expired before the balance check finished. Sign in again before checking balances.";
  }
  if (lower.includes("returned no wallets")) {
    return "We could not find a wallet for this account, so wallet cash is unknown.";
  }
  if (lower.includes("gateway") || lower.includes("circle")) {
    return "Wallet balance check failed.";
  }
  if (lower.includes("failed to fetch") || lower.includes("networkerror")) {
    return "We could not check balances. Check your connection and try again.";
  }
  return message;
}

export function friendlyAccountError(error: unknown) {
  const message = error instanceof Error ? error.message.toLowerCase() : "";
  if (message.includes("funds_present")) {
    return "Move your funds out before closing your account.";
  }
  if (message.includes("email_in_use") || message.includes("already in use")) {
    return "That email is already in use.";
  }
  if (
    message.includes("email_unchanged") ||
    message.includes("different email")
  ) {
    return "Enter a different email address.";
  }
  if (message.includes("code")) {
    return "That code did not work. Check it or request a new one.";
  }
  if (message.includes("export email is not configured")) {
    return "We could not prepare your export email. Try again later.";
  }
  if (message.includes("balance cannot be verified")) {
    return "We could not verify balances. Try again later.";
  }
  if (message.includes("401") || message.includes("unauthorized")) {
    return "Your session expired. Enter your email to continue.";
  }
  return "Something went wrong. Try again.";
}
