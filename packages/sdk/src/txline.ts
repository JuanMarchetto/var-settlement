/**
 * Tx LINE client for VAR.
 *
 * Encodes the verified free-World-Cup access flow and the `stat-validation` Merkle-proof fetch,
 * and shapes the response into the `StatWitness` the on-chain `resolve` instruction expects.
 *
 * Verified against txline.txodds.com/documentation (2026-07-05). Re-confirm on first connect.
 */

import { BN } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import axios, { AxiosInstance } from "axios";
import nacl from "tweetnacl";

export type Network = "mainnet" | "devnet";

export const TXLINE = {
  mainnet: {
    rpcUrl: "https://api.mainnet-beta.solana.com",
    apiOrigin: "https://txline.txodds.com",
    programId: new PublicKey("9ExbZjAapQww1vfcisDmrngPinHTEfpjYRWMunJgcKaA"),
    // Service level 12 = real-time, mainnet only.
    defaultServiceLevel: 12,
  },
  devnet: {
    rpcUrl: "https://api.devnet.solana.com",
    apiOrigin: "https://txline-dev.txodds.com",
    programId: new PublicKey("6pW64gN1s2uqjHkn1unFeEjAwJkPGHoppGvS715wyP2J"),
    // Service level 1 = 60s-delayed, works on devnet.
    defaultServiceLevel: 1,
  },
} as const;

/** 32-byte value from the API: "0x…" hex, base64, or a number[]. */
export function toBytes32(value: string | number[] | Uint8Array): number[] {
  const bytes = Array.isArray(value)
    ? Uint8Array.from(value)
    : value instanceof Uint8Array
    ? value
    : value.startsWith("0x")
    ? Buffer.from(value.slice(2), "hex")
    : Buffer.from(value, "base64");
  if (bytes.length !== 32) throw new Error(`expected 32 bytes, got ${bytes.length}`);
  return Array.from(bytes);
}

export interface ProofNode {
  hash: number[]; // [u8; 32]
  isRightSibling: boolean;
}
function toProofNodes(
  nodes: Array<{ hash: string | number[] | Uint8Array; isRightSibling: boolean }>
): ProofNode[] {
  return nodes.map((n) => ({ hash: toBytes32(n.hash), isRightSibling: n.isRightSibling }));
}

/** Derive the daily-scores-roots PDA: seeds ["daily_scores_roots", (epochDay as u16) LE]. */
export function dailyScoresRootsPda(programId: PublicKey, tsMillis: number): [PublicKey, number] {
  const epochDay = Math.floor(tsMillis / 86_400_000);
  return PublicKey.findProgramAddressSync(
    [Buffer.from("daily_scores_roots"), new BN(epochDay).toArrayLike(Buffer, "le", 2)],
    programId
  );
}

/**
 * A `StatWitness` matching `var_settlement::StatWitness` — authenticates one score stat.
 * `goals` (the value) is passed on-chain as the `EqualTo` predicate threshold.
 */
export interface StatWitness {
  ts: BN;
  summary: {
    fixtureId: BN;
    updateStats: { updateCount: number; minTimestamp: BN; maxTimestamp: BN };
    eventsSubTreeRoot: number[];
  };
  fixtureProof: ProofNode[];
  mainTreeProof: ProofNode[];
  stat: {
    statToProve: { key: number; value: number; period: number };
    eventStatRoot: number[];
    statProof: ProofNode[];
  };
}

export class TxLineClient {
  readonly cfg: (typeof TXLINE)[Network];
  private http: AxiosInstance | null = null;

  constructor(readonly network: Network) {
    this.cfg = TXLINE[network];
  }

  /**
   * 4-step self-serve activation for the free World Cup tier.
   * NOTE: `subscribeTxSig` must be the signature of an on-chain `Txoracle::subscribe(serviceLevel,
   * durationWeeks)` transaction already sent by `wallet` on the SAME network. See `subscribe()`.
   */
  async activate(wallet: Keypair, subscribeTxSig: string, leagues = "worldcup"): Promise<void> {
    const origin = this.cfg.apiOrigin;
    // (2) guest JWT
    const { data: guest } = await axios.post(`${origin}/auth/guest/start`, {});
    const jwt: string = guest.jwt ?? guest.token;
    // (3) sign `${txSig}:${leagues}:${jwt}` (Ed25519 / NaCl)
    const message = `${subscribeTxSig}:${leagues}:${jwt}`;
    const signature = nacl.sign.detached(Buffer.from(message), wallet.secretKey);
    // (4) activate -> X-Api-Token
    const { data: act } = await axios.post(
      `${origin}/api/token/activate`,
      {
        txSignature: subscribeTxSig,
        leagues,
        pubkey: wallet.publicKey.toBase58(),
        signature: Buffer.from(signature).toString("base64"),
      },
      { headers: { Authorization: `Bearer ${jwt}` } }
    );
    const apiToken: string = act.apiToken ?? act.token ?? act["X-Api-Token"];
    this.http = axios.create({
      baseURL: origin,
      timeout: 30_000,
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${jwt}`,
        "X-Api-Token": apiToken,
        "Accept-Encoding": "gzip",
      },
    });
  }

  private client(): AxiosInstance {
    if (!this.http) throw new Error("call activate() before fetching data");
    return this.http;
  }

  /** Latest scores snapshot for a fixture. */
  async scoresSnapshot(fixtureId: number, asOf = Date.now()): Promise<any> {
    const { data } = await this.client().get(`/api/scores/snapshot/${fixtureId}?asOf=${asOf}`);
    return data;
  }

  /**
   * Fetch a stat-validation payload and shape it into a `StatWitness`. `statKey`/`seq` identify
   * the score stat (e.g. final home goals). Validate authenticity on-chain via `validate_stat`.
   */
  async statWitness(fixtureId: number, seq: number, statKey: number): Promise<StatWitness> {
    const { data: v } = await this.client().get(`/api/scores/stat-validation`, {
      params: { fixtureId, seq, statKey },
    });
    return {
      ts: new BN(v.summary.updateStats.minTimestamp),
      summary: {
        fixtureId: new BN(v.summary.fixtureId),
        updateStats: {
          updateCount: v.summary.updateStats.updateCount,
          minTimestamp: new BN(v.summary.updateStats.minTimestamp),
          maxTimestamp: new BN(v.summary.updateStats.maxTimestamp),
        },
        eventsSubTreeRoot: toBytes32(v.summary.eventStatsSubTreeRoot),
      },
      fixtureProof: toProofNodes(v.subTreeProof),
      mainTreeProof: toProofNodes(v.mainTreeProof),
      stat: {
        statToProve: {
          key: v.statToProve.key,
          value: v.statToProve.value,
          period: v.statToProve.period,
        },
        eventStatRoot: toBytes32(v.eventStatRoot),
        statProof: toProofNodes(v.statProof),
      },
    };
  }
}

/**
 * Build the on-chain `subscribe(serviceLevel, durationWeeks)` transaction for the free tier.
 * Left as a thin wrapper because the exact account set comes from the Txoracle IDL
 * (docs/idl/txoracle_mainnet.json). Wire this against the generated program client on first connect.
 */
export async function subscribe(
  _connection: Connection,
  _wallet: Keypair,
  _network: Network,
  _serviceLevel: number,
  _durationWeeks: number
): Promise<string> {
  throw new Error(
    "subscribe(): wire against the Txoracle IDL on first connect (needs a funded wallet + small SOL)."
  );
}
