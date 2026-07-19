/**
 * Tx LINE free World Cup tier activation on devnet (Phase A of the live settlement).
 *
 * subscribe(serviceLevel=1, weeks=4) on the real Txoracle -> guest JWT -> sign -> /token/activate.
 * Saves { jwt, apiToken } to .txline-creds.json for the settlement step.
 */
import * as anchor from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountIdempotent,
} from "@solana/spl-token";
import nacl from "tweetnacl";
import { readFileSync, writeFileSync } from "fs";

const RPC = "https://api.devnet.solana.com";
const API_ORIGIN = "https://txline-dev.txodds.com";
const PROGRAM_ID = new PublicKey("6pW64gN1s2uqjHkn1unFeEjAwJkPGHoppGvS715wyP2J");
const TXL_MINT = new PublicKey("4Zao8ocPhmMgq7PdsYWyxvqySMGx7xb9cMftPMkEokRG");
const SERVICE_LEVEL_ID = 1; // World Cup & Int Friendlies, 60s-delayed (devnet)
const DURATION_WEEKS = 4;
const LEAGUES: number[] = []; // standard bundle

function loadKp(p: string): Keypair {
  return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(p, "utf8"))));
}

async function main() {
  const conn = new Connection(RPC, "confirmed");
  const payer = loadKp(`${process.env.HOME}/.config/solana/var-settlement-deployer.json`);
  const wallet = new anchor.Wallet(payer);
  const provider = new anchor.AnchorProvider(conn, wallet, { commitment: "confirmed" });
  anchor.setProvider(provider);
  const idl = JSON.parse(readFileSync(new URL("../idl/txoracle_devnet.json", import.meta.url), "utf8"));
  const program = new anchor.Program(idl, provider);

  // PDAs + ATAs (TxL is Token-2022).
  const [tokenTreasuryPda] = PublicKey.findProgramAddressSync([Buffer.from("token_treasury_v2")], PROGRAM_ID);
  const [pricingMatrixPda] = PublicKey.findProgramAddressSync([Buffer.from("pricing_matrix")], PROGRAM_ID);
  const tokenTreasuryVault = getAssociatedTokenAddressSync(TXL_MINT, tokenTreasuryPda, true, TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID);
  const userTokenAccount = getAssociatedTokenAddressSync(TXL_MINT, payer.publicKey, false, TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID);

  console.log("== ensure user TxL ATA (Token-2022) ==");
  await createAssociatedTokenAccountIdempotent(conn, payer, TXL_MINT, payer.publicKey, {}, TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID);
  console.log(`  userTokenAccount: ${userTokenAccount.toBase58()}`);

  // Reuse an existing subscribe tx (TXSIG=...) to avoid re-subscribing; else subscribe now.
  let txSig = process.env.TXSIG || "";
  if (txSig) {
    console.log(`== reusing subscribe tx: ${txSig} ==`);
  } else {
    console.log("== subscribe(1, 4) on Txoracle ==");
    txSig = await program.methods
      .subscribe(SERVICE_LEVEL_ID, DURATION_WEEKS)
      .accounts({
        user: payer.publicKey,
        pricingMatrix: pricingMatrixPda,
        tokenMint: TXL_MINT,
        userTokenAccount,
        tokenTreasuryVault,
        tokenTreasuryPda,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log(`  subscribe tx: ${txSig}`);
  }

  console.log("== guest JWT + sign + activate ==");
  const jwt: string = (await (await fetch(`${API_ORIGIN}/auth/guest/start`, {
    method: "POST", headers: { "Accept-Encoding": "gzip" },
  })).json()).token;
  const messageString = `${txSig}:${LEAGUES.join(",")}:${jwt}`;
  const signature = nacl.sign.detached(new TextEncoder().encode(messageString), payer.secretKey);
  const walletSignature = Buffer.from(signature).toString("base64");

  const actRes = await fetch(`${API_ORIGIN}/api/token/activate`, {
    method: "POST",
    headers: { "Content-Type": "application/json", Authorization: `Bearer ${jwt}`, "Accept-Encoding": "gzip" },
    body: JSON.stringify({ txSig, walletSignature, leagues: LEAGUES }),
  });
  const raw = await actRes.text();
  console.log(`  activate: status=${actRes.status} enc=${actRes.headers.get("content-encoding")} ct=${actRes.headers.get("content-type")}`);
  if (!actRes.ok) throw new Error(`activate HTTP ${actRes.status}: ${raw.slice(0, 200)}`);
  // The endpoint returns the API token as plain text (or JSON on some hosts).
  let apiToken: string;
  try {
    const j = JSON.parse(raw);
    apiToken = j.token || j.apiToken || String(j);
  } catch {
    apiToken = raw.trim();
  }
  // Never print the token itself — this output gets pasted into docs/demos.
  console.log(`  API token: received (${String(apiToken).length} chars) -> .txline-creds.json`);

  writeFileSync(new URL("./.txline-creds.json", import.meta.url), JSON.stringify({ jwt, apiToken, txSig }, null, 2));
  console.log("\nACTIVATION OK -> saved .txline-creds.json");
}

main().catch((e) => { console.error("ACTIVATION FAILED:", e?.message || e); process.exit(1); });
