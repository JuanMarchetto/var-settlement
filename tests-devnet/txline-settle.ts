/**
 * REAL settlement (Phase 2): settle a VAR market for a real World Cup fixture against the LIVE
 * Tx LINE feed. Fetches authentic stat-validation Merkle proofs for the home/away goals, then
 * resolves by CPI into the real Txoracle::validate_stat over the on-chain daily root.
 *
 * Usage: bun run txline-settle.ts <fixtureId> <seq>
 */
import * as anchor from "@coral-xyz/anchor";
import { BN } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, SystemProgram, SYSVAR_RENT_PUBKEY, ComputeBudgetProgram } from "@solana/web3.js";
import {
  createMint, getOrCreateAssociatedTokenAccount, mintTo, getAccount, TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { readFileSync } from "fs";

const RPC = "https://api.devnet.solana.com";
const API = "https://txline-dev.txodds.com/api";
const VAR_PROGRAM = new PublicKey("AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk");
const TXORACLE_REAL = new PublicKey("6pW64gN1s2uqjHkn1unFeEjAwJkPGHoppGvS715wyP2J");
const U = 1_000_000;

const { jwt, apiToken } = JSON.parse(readFileSync(new URL("./.txline-creds.json", import.meta.url), "utf8"));
const H = { Authorization: `Bearer ${jwt}`, "X-Api-Token": apiToken, "Accept-Encoding": "gzip" };
const loadKp = (p: string) => Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(p, "utf8"))));

async function statValidation(fixtureId: number, seq: number, statKey: number): Promise<any> {
  const r = await fetch(`${API}/scores/stat-validation?fixtureId=${fixtureId}&seq=${seq}&statKey=${statKey}`, { headers: H });
  if (!r.ok) throw new Error(`stat-validation ${statKey} HTTP ${r.status}: ${(await r.text()).slice(0, 200)}`);
  return JSON.parse(await r.text());
}
const nodes = (a: any[]) => a.map((n) => ({ hash: n.hash, isRightSibling: n.isRightSibling }));
function toWitness(v: any) {
  return {
    ts: new BN(v.summary.updateStats.minTimestamp),
    summary: {
      fixtureId: new BN(v.summary.fixtureId),
      updateStats: {
        updateCount: v.summary.updateStats.updateCount,
        minTimestamp: new BN(v.summary.updateStats.minTimestamp),
        maxTimestamp: new BN(v.summary.updateStats.maxTimestamp),
      },
      eventsSubTreeRoot: v.summary.eventStatsSubTreeRoot,
    },
    fixtureProof: nodes(v.subTreeProof),
    mainTreeProof: nodes(v.mainTreeProof),
    stat: { statToProve: v.statToProve, eventStatRoot: v.eventStatRoot, statProof: nodes(v.statProof) },
  };
}

async function main() {
  const fixtureId = Number(process.argv[2] || 18192996);
  const seq = Number(process.argv[3] || 770);
  const conn = new Connection(RPC, "confirmed");
  const deployer = loadKp(`${process.env.HOME}/.config/solana/var-settlement-deployer.json`);
  const provider = new anchor.AnchorProvider(conn, new anchor.Wallet(deployer), { commitment: "confirmed" });
  anchor.setProvider(provider);
  const idl = JSON.parse(readFileSync(new URL("../idl/var_settlement.json", import.meta.url), "utf8"));
  const program = new anchor.Program(idl, provider);

  console.log(`== fetch REAL proofs for fixture ${fixtureId} seq ${seq} ==`);
  const vHome = await statValidation(fixtureId, seq, 1); // base key 1 = Participant1 (home) goals
  const vAway = await statValidation(fixtureId, seq, 2); // base key 2 = Participant2 (away) goals
  const homeGoals = vHome.statToProve.value, awayGoals = vAway.statToProve.value;
  const period = vHome.statToProve.period;
  console.log(`  authenticated score: home ${homeGoals} - ${awayGoals} away (statKey period ${period})`);
  if (vAway.statToProve.period !== period) throw new Error("home/away stat periods differ; need one market.period");

  // daily_scores_roots PDA on the REAL Txoracle for this fixture's epoch day.
  const minTs = vHome.summary.updateStats.minTimestamp;
  const epochDay = Math.floor(minTs / 86_400_000);
  const [dailyPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("daily_scores_roots"), new BN(epochDay).toArrayLike(Buffer, "le", 2)], TXORACLE_REAL);
  console.log(`  epochDay ${epochDay} -> daily_scores_roots PDA ${dailyPda.toBase58()}`);
  const rootAcct = await conn.getAccountInfo(dailyPda);
  console.log(`  daily root account on-chain: ${rootAcct ? `EXISTS (${rootAcct.data.length} bytes)` : "MISSING"}`);

  // Market + escrow (test USDC mint stands in for USDC; the settlement proof is what's real).
  console.log("\n== setup market + deposits ==");
  const usdc = await createMint(conn, deployer, deployer.publicKey, null, 6);
  const A = Keypair.generate(), B = Keypair.generate();
  for (const u of [A, B]) await (await import("@solana/web3.js")).sendAndConfirmTransaction(conn,
    new (await import("@solana/web3.js")).Transaction().add(SystemProgram.transfer({ fromPubkey: deployer.publicKey, toPubkey: u.publicKey, lamports: 0.05 * 1e9 })), [deployer]);
  const ataA = (await getOrCreateAssociatedTokenAccount(conn, deployer, usdc, A.publicKey)).address;
  const ataB = (await getOrCreateAssociatedTokenAccount(conn, deployer, usdc, B.publicKey)).address;
  await mintTo(conn, deployer, usdc, ataA, deployer, 100 * U);
  await mintTo(conn, deployer, usdc, ataB, deployer, 100 * U);

  const fidBN = new BN(fixtureId);
  const marketKind = (Math.floor(Date.now() / 1000) % 250) + 1; // nonce so re-runs get a fresh market
  const [market] = PublicKey.findProgramAddressSync([Buffer.from("market"), fidBN.toArrayLike(Buffer, "le", 8), Buffer.from([marketKind])], VAR_PROGRAM);
  const [vault] = PublicKey.findProgramAddressSync([Buffer.from("vault"), market.toBuffer()], VAR_PROGRAM);
  const posA = PublicKey.findProgramAddressSync([Buffer.from("position"), market.toBuffer(), A.publicKey.toBuffer()], VAR_PROGRAM)[0];
  const posB = PublicKey.findProgramAddressSync([Buffer.from("position"), market.toBuffer(), B.publicKey.toBuffer()], VAR_PROGRAM)[0];

  await program.methods.createMarket({
    fixtureId: fidBN, marketKind, homeStatKey: vHome.statToProve.key, awayStatKey: vAway.statToProve.key,
    period, feeBps: 200, resolveDeadline: new BN(Math.floor(Date.now() / 1000) + 3600),
  }).accounts({ creator: deployer.publicKey, market, usdcMint: usdc, vault, tokenProgram: TOKEN_PROGRAM_ID, systemProgram: SystemProgram.programId, rent: SYSVAR_RENT_PUBKEY }).rpc();
  await program.methods.deposit(0, new BN(40 * U)).accounts({ depositor: A.publicKey, market, vault, depositorAta: ataA, position: posA, tokenProgram: TOKEN_PROGRAM_ID, systemProgram: SystemProgram.programId }).signers([A]).rpc();
  await program.methods.deposit(2, new BN(60 * U)).accounts({ depositor: B.publicKey, market, vault, depositorAta: ataB, position: posB, tokenProgram: TOKEN_PROGRAM_ID, systemProgram: SystemProgram.programId }).signers([B]).rpc();
  console.log("  market created; A->Home 40, B->Away 60");

  console.log("\n== RESOLVE against the REAL Txoracle (two-step CPI validate_stat) ==");
  const oracleAccts = { resolver: deployer.publicKey, market, dailyScoresMerkleRoots: dailyPda, txoracleProgram: TXORACLE_REAL };
  // validate_stat Merkle verification is compute-heavy; raise the CU limit (docs use 1.4M).
  const cuIx = ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 });
  const sigHome = await program.methods.attestHome(toWitness(vHome)).accounts(oracleAccts).preInstructions([cuIx]).rpc();
  console.log(`  attest_home tx: ${sigHome}`);
  const sig = await program.methods.resolve(toWitness(vAway), 0).accounts(oracleAccts).preInstructions([cuIx]).rpc();
  console.log(`  resolve tx: ${sig}`);
  const m = await program.account.market.fetch(market);
  const oc = ["Home", "Draw", "Away", "Refund"][m.receipt.outcomeCode];
  console.log(`  receipt: outcome=${oc} paidAsRefund=${m.receipt.paidAsRefund} net=${m.receipt.net.toString()} winningPool=${m.receipt.winningPool.toString()}`);

  const verified = await program.methods.reverify().accounts({ market }).view();
  console.log(`\n== reverify -> ${verified} ==`);

  await program.methods.claim().accounts({ claimant: A.publicKey, market, position: posA, vault, vaultAuthority: vault, recipientAta: ataA, tokenProgram: TOKEN_PROGRAM_ID }).signers([A]).rpc();
  await program.methods.claim().accounts({ claimant: B.publicKey, market, position: posB, vault, vaultAuthority: vault, recipientAta: ataB, tokenProgram: TOKEN_PROGRAM_ID }).signers([B]).rpc();
  const balA = Number((await getAccount(conn, ataA)).amount) / U, balB = Number((await getAccount(conn, ataB)).amount) / U;
  const expectAway = awayGoals > homeGoals;
  console.log(`\n== balances == A(Home)=${balA} B(Away)=${balB}  (Away won: ${expectAway})`);
  const pass = verified === true && oc === (awayGoals > homeGoals ? "Away" : awayGoals < homeGoals ? "Home" : "Draw");
  console.log(`\n${pass ? "REAL SETTLEMENT PASSED" : "SETTLEMENT MISMATCH"}  (resolve tx: ${sig})`);
  process.exit(pass ? 0 : 1);
}
main().catch((e) => { console.error("SETTLE FAILED:", e?.message || e); process.exit(1); });
