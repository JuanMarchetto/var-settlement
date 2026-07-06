/**
 * VAR devnet end-to-end smoke test.
 *
 * Drives the real deployed program through the full lifecycle against a mock Txoracle:
 *   create_market -> deposit (Home & Away) -> resolve (Home wins) -> reverify -> claim.
 * Proves the on-chain settlement flow works with real transactions on devnet.
 */
import * as anchor from "@coral-xyz/anchor";
import { BN } from "@coral-xyz/anchor";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
  Transaction,
  sendAndConfirmTransaction,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  getAccount,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { readFileSync } from "fs";

const VAR_PROGRAM = new PublicKey("AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk");
const MOCK_TXORACLE = new PublicKey("85KwDRzyZeG8wAXVCZo2CKTVor3qVcyhq7vk2yAzBJMw");
const U = 1_000_000; // 6-decimal token units

function loadKp(path: string): Keypair {
  return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(path, "utf8"))));
}
const zero32 = () => Array(32).fill(0);
const ok = (label: string, sig: string) => console.log(`  OK ${label}: ${sig}`);

async function fund(conn: Connection, from: Keypair, to: PublicKey, sol: number) {
  const tx = new Transaction().add(
    SystemProgram.transfer({ fromPubkey: from.publicKey, toPubkey: to, lamports: sol * LAMPORTS_PER_SOL })
  );
  await sendAndConfirmTransaction(conn, tx, [from]);
}

async function main() {
  const conn = new Connection("https://api.devnet.solana.com", "confirmed");
  const deployer = loadKp(`${process.env.HOME}/.config/solana/var-settlement-deployer.json`);
  const wallet = new anchor.Wallet(deployer);
  const provider = new anchor.AnchorProvider(conn, wallet, { commitment: "confirmed" });
  anchor.setProvider(provider);
  const idl = JSON.parse(readFileSync(new URL("../idl/var_settlement.json", import.meta.url), "utf8"));
  const program = new anchor.Program(idl, provider);

  console.log("== setup ==");
  const usdc = await createMint(conn, deployer, deployer.publicKey, null, 6);
  console.log(`  test USDC mint: ${usdc.toBase58()}`);
  const userA = Keypair.generate();
  const userB = Keypair.generate();
  await fund(conn, deployer, userA.publicKey, 0.05);
  await fund(conn, deployer, userB.publicKey, 0.05);
  const ataA = (await getOrCreateAssociatedTokenAccount(conn, deployer, usdc, userA.publicKey)).address;
  const ataB = (await getOrCreateAssociatedTokenAccount(conn, deployer, usdc, userB.publicKey)).address;
  await mintTo(conn, deployer, usdc, ataA, deployer, 100 * U);
  await mintTo(conn, deployer, usdc, ataB, deployer, 100 * U);
  console.log("  userA/userB funded with 100 test-USDC each");

  const fixtureId = new BN(Math.floor(Date.now() / 1000)); // unique per run so the market PDA is fresh
  const marketKind = 0;
  const homeStatKey = 1002;
  const awayStatKey = 1003;
  const period = 0;
  const feeBps = 200;
  const resolveDeadline = new BN(Math.floor(Date.now() / 1000) + 3600);

  const [market] = PublicKey.findProgramAddressSync(
    [Buffer.from("market"), fixtureId.toArrayLike(Buffer, "le", 8), Buffer.from([marketKind])],
    VAR_PROGRAM
  );
  const [vault] = PublicKey.findProgramAddressSync([Buffer.from("vault"), market.toBuffer()], VAR_PROGRAM);
  const posA = PublicKey.findProgramAddressSync(
    [Buffer.from("position"), market.toBuffer(), userA.publicKey.toBuffer()], VAR_PROGRAM)[0];
  const posB = PublicKey.findProgramAddressSync(
    [Buffer.from("position"), market.toBuffer(), userB.publicKey.toBuffer()], VAR_PROGRAM)[0];

  console.log("\n== create_market ==");
  ok("create_market", await program.methods
    .createMarket({ fixtureId, marketKind, homeStatKey, awayStatKey, period, feeBps, resolveDeadline })
    .accounts({
      creator: deployer.publicKey, market, usdcMint: usdc, vault,
      tokenProgram: TOKEN_PROGRAM_ID, systemProgram: SystemProgram.programId, rent: SYSVAR_RENT_PUBKEY,
    }).rpc());

  console.log("\n== deposit ==");
  ok("A -> Home 40", await program.methods.deposit(0, new BN(40 * U))
    .accounts({ depositor: userA.publicKey, market, vault, depositorAta: ataA, position: posA,
      tokenProgram: TOKEN_PROGRAM_ID, systemProgram: SystemProgram.programId })
    .signers([userA]).rpc());
  ok("B -> Away 60", await program.methods.deposit(2, new BN(60 * U))
    .accounts({ depositor: userB.publicKey, market, vault, depositorAta: ataB, position: posB,
      tokenProgram: TOKEN_PROGRAM_ID, systemProgram: SystemProgram.programId })
    .signers([userB]).rpc());

  // Witnesses: mock Txoracle attests anything, but VAR still binds fixtureId/statKey/period.
  const witness = (statKey: number, value: number) => ({
    ts: new BN(Date.now()),
    summary: {
      fixtureId,
      updateStats: { updateCount: 1, minTimestamp: new BN(Date.now()), maxTimestamp: new BN(Date.now()) },
      eventsSubTreeRoot: zero32(),
    },
    fixtureProof: [],
    mainTreeProof: [],
    stat: { statToProve: { key: statKey, value, period }, eventStatRoot: zero32(), statProof: [] },
  });

  console.log("\n== resolve (Home 2 - 0 Away, Completed) ==");
  ok("resolve", await program.methods
    .resolve(witness(homeStatKey, 2), witness(awayStatKey, 0), 0)
    .accounts({ resolver: deployer.publicKey, market,
      dailyScoresMerkleRoots: SystemProgram.programId, txoracleProgram: MOCK_TXORACLE })
    .rpc());

  const m = await program.account.market.fetch(market);
  console.log(`  receipt: outcome=${m.receipt.outcomeCode} (0=Home) paidAsRefund=${m.receipt.paidAsRefund} net=${m.receipt.net.toString()} winningPool=${m.receipt.winningPool.toString()}`);

  console.log("\n== reverify (permissionless re-derivation) ==");
  const verified = await program.methods.reverify().accounts({ market }).view();
  console.log(`  reverify -> ${verified}`);

  console.log("\n== claim ==");
  ok("A claims (Home winner)", await program.methods.claim()
    .accounts({ claimant: userA.publicKey, market, position: posA, vault, vaultAuthority: vault,
      recipientAta: ataA, tokenProgram: TOKEN_PROGRAM_ID }).signers([userA]).rpc());
  ok("B claims (Away loser)", await program.methods.claim()
    .accounts({ claimant: userB.publicKey, market, position: posB, vault, vaultAuthority: vault,
      recipientAta: ataB, tokenProgram: TOKEN_PROGRAM_ID }).signers([userB]).rpc());

  const balA = Number((await getAccount(conn, ataA)).amount) / U;
  const balB = Number((await getAccount(conn, ataB)).amount) / U;
  const balV = Number((await getAccount(conn, vault)).amount) / U;
  console.log(`\n== balances ==\n  userA: ${balA} (expect 158)  userB: ${balB} (expect 40)  vault: ${balV} (expect 2 fee)`);

  const pass = verified === true && balA === 158 && balB === 40 && balV === 2 && m.receipt.outcomeCode === 0;
  console.log(`\n${pass ? "SMOKE TEST PASSED" : "SMOKE TEST FAILED"}`);
  process.exit(pass ? 0 : 1); // exit immediately so trailing RPC retries don't affect the exit code
}

main().catch((e) => { console.error(e); process.exit(1); });
