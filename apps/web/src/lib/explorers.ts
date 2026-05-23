export type ExplorerChain =
  | "arc"
  | "base"
  | "eth-sepolia"
  | "arb-sepolia"
  | "avax-fuji";

const EXPLORER_BASE_URL: Record<ExplorerChain, string> = {
  arc: "https://testnet.arcscan.app",
  base: "https://sepolia.basescan.org",
  "eth-sepolia": "https://sepolia.etherscan.io",
  "arb-sepolia": "https://sepolia.arbiscan.io",
  "avax-fuji": "https://testnet.snowtrace.io",
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
