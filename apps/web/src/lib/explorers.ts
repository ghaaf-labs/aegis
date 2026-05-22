export type ExplorerChain = "arc" | "base";

const EXPLORER_BASE_URL: Record<ExplorerChain, string> = {
  arc: "https://testnet.arcscan.app",
  base: "https://sepolia.basescan.org",
};

export function explorerTxUrl(
  chain: ExplorerChain | null | undefined,
  txHash: string | null | undefined,
) {
  if (!chain || !txHash) return null;
  return `${EXPLORER_BASE_URL[chain]}/tx/${txHash}`;
}

export function explorerAddressUrl(
  chain: ExplorerChain | null | undefined,
  address: string | null | undefined,
) {
  if (!chain || !address) return null;
  return `${EXPLORER_BASE_URL[chain]}/address/${address}`;
}
