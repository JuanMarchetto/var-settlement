/**
 * VAR SDK entrypoint.
 *
 * Re-exports the Tx LINE client and witness helpers. A `resolve` call on the VAR program takes two
 * `StatWitness` values (home goals, away goals), each assembled from `TxLineClient.statWitness(...)`,
 * plus the `daily_scores_merkle_roots` PDA from `dailyScoresRootsPda(...)`.
 */

export {
  TxLineClient,
  TXLINE,
  dailyScoresRootsPda,
  toBytes32,
  subscribe,
} from "./txline.js";
export type { Network, ProofNode, StatWitness } from "./txline.js";
