// Client-safe constants & types (NO database import — safe to use in client
// components without dragging the Neon client / env access into the browser).

export type RunRow = {
  run_id: string;
  started_at: number | null;
  captured_at: number | null;
  status: string | null;
  client: string | null;
  image: string | null;
  devnet: string | null;
  invocation: string | null;
  namespace: string | null;
  context: string | null;
  host_map: any;
  flags: any;
  genesis: any;
  fixes: any;
  outcome: any;
  notes: string | null;
  source: string | null;
};

/** The fix facets, in display order. */
export const FIX_KEYS = [
  "proving_conjectured",
  "gossip_disparity",
  "block_builder_prefilter",
  "target_guard",
  "offloop_aggregation",
  "headstate_finalized",
] as const;

export const FIX_LABELS: Record<string, string> = {
  proving_conjectured: "conjectured",
  gossip_disparity: "gossip-disparity",
  block_builder_prefilter: "block-builder",
  target_guard: "target-guard",
  offloop_aggregation: "off-loop-agg",
  headstate_finalized: "head-state-fin",
};
