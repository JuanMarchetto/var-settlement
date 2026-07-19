/**
 * VAR SDK entrypoint.
 *
 * Re-exports the Tx LINE client and witness helpers. Settlement takes two `StatWitness` values
 * (home goals via `attest_home`, away goals via `resolve` — two instructions because both Merkle
 * proofs exceed the 1232-byte tx limit), each assembled from `TxLineClient.statWitness(...)`,
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
