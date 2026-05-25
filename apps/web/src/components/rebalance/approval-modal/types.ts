import type {
  DeferredTarget,
  RebalanceApprovalSafety,
  RebalancePlanResponse,
} from "@/lib/api";
import type { AgentDecision } from "@/types";

export interface ApprovalModalProps {
  open: boolean;
  plan: RebalancePlanResponse | null;
  /** Sleeves the plan intended but couldn't route now — held as USDC reserve
   *  and shown as intent so the review reflects the full target (spec §12). */
  deferred?: DeferredTarget[];
  /** Drives the inline backtest preview. Defaults to no preview when null. */
  portfolioId?: string | null;
  estimatedFeeUsdc: number;
  /** When the fee number was fetched. Drives the provenance line. */
  feeFetchedAt?: Date | null;
  /** Where the fee came from — `plan` is the planner-time stored value;
   *  `paymaster` is a live quote from `GET /paymaster/estimate`. */
  feeSource?: "plan" | "paymaster";
  /** Optional per-user / per-portfolio context surfaced in the header. */
  portfolioName?: string;
  /** The AgentDecision behind this plan. When present the modal surfaces
   *  model_slug + confidence + critic verdict next to the plan — required
   *  for Agentic Sophistication judging (30% weight). */
  decision?: AgentDecision | null;
  approvalSafety?: RebalanceApprovalSafety | null;
  onApproved: (rebalanceId: string) => void;
  onClose: () => void;
}
